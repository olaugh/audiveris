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

The CLI now performs native schema-1 JSON recognition through HEADS. GRID's
human-readable report remains unchanged; HEADERS, STEM_SEEDS, BEAMS, LEDGERS,
and HEADS require `-json` and compose in Java stage order rather than accepting
invented downstream inputs. HEADS runs GRID -> HEADERS -> STEM_SEEDS -> BEAMS
-> LEDGERS -> HEADS, retains every upstream product, and adds identity-free
final heads, source provenance, exact glyph evidence, beam decisions, and
tally-scale rows without fabricating Java SIG or glyph IDs.

`omrscope` now compares the two producers while they run: Rust and Java start
independently, each publishes an immutable snapshot once it completes GRID,
HEADERS, STEM_SEEDS, BEAMS, LEDGERS, or HEADS, and the viewer can select any
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
| STEMS beam-V reuse/check boundary | `evaluate_native_stems_beam_vlink_reuse_check` is the thirteenth exact, read-only boundary: it preserves the ordered/lazy head-side stem-reuse loop and exact `BeamStemRelation.checkLink`. All 65 real C entries are unlinked, all 30 checks accept, and real reuse is zero; eight isolated synthetic SIG blocks cover the reuse and numerical branches. It stops at `ReadyBeforeSigMutation` |
| STEMS beam-V base SIG/BeamStem application | `apply_native_stems_beam_vlink_base_transaction` is the fourteenth exact boundary: it conditionally executes `SIG.addVertex` and then applies the checked base BeamStem relation with exact index/SIG ordering, duplicate suppression, JGraphT callbacks, abnormal/dirty effects, and partial prefixes. All 30 real rows are `NewIdZero` with vertex and edge added, reuse zero, and zero ChordStem matches; 40 supported and 32 envelope cases are isolated evidence, not production-equivalence. Twenty focused units and the 10/10 33.87-second gate are green; it stops at `ReadyBeforeBLinkerFlagMutation` |
| STEMS beam-V B-linker shared flag assignment | `apply_native_stems_beam_vlink_b_linker_flag_transaction` is the fifteenth exact boundary: it independently reruns boundary 14 from its exact pre-state, resolves the scheduler-selected outer B and its TOP-then-BOTTOM V observers, and executes one plain `linked = true` assignment while retaining the ignored base-link return and fresh draft grade. All 30 real writes change false to true across a guarded Java arena of 3,948 entries (2,116 frozen + 1,832 dynamic anchors). Eight isolated blocks add 32 setter/shared-cell-only Unsafe exact-class envelopes: 24 false-to-true, 8 idempotent, and 8 with `applyReturn=false`. Seven focused units and the shared 5/5 126.03-second hydration regression are green; it stops at `ReadyBeforeSiblingBeamLinks` |
| STEMS beam-V sibling BeamStem links | `apply_native_stems_beam_vlink_sibling_links_transaction` is the sixteenth exact boundary: it exact-replays Boundary 15 and executes the complete serial sibling loop through group ordering, identity/duplicate/shorter branches, fresh BeamStem edge callbacks, zero-Chord validation, beam abnormal/dirty effects, and optional sibling B-cell writes. The 30 real transactions expose 58 non-null native-glyph group members and 11 siblings; all 11 link, add one edge, and write one B cell. Eight isolated blocks add 64 supported branch cases and 16 Java throw envelopes without claiming production equivalence. Twenty-two focused units and the 10/10 126.68-second full exact gate are green; it stops at `ReadyBeforeHeadRelationLoop` |
| STEMS beam-V head relations | `apply_native_stems_beam_vlink_head_links_transaction` is the seventeenth exact boundary: it exact-replays Boundary 16 and executes the insertion-ordered head map through unconditional shared S-cell assignment, exact directed duplicate lookup, lazy consistency mutation of the existing plan draft, direct HeadStem insertion, and synchronous head/stem abnormal and dirty callbacks. The 30 real transactions contain 65 entries, zero duplicates, 65 inserts, 65 S-cell and consistency writes, and 260 ordered events. Eight isolated blocks add 16 supported and 40 envelope transactions without claiming production equivalence. Twenty-four focused units, the 13/13 148.82-second full exact gate, and the 1/1 129.11-second manifest validator are green; it stops at `ReturnedTrueBeforeOuterBLinkerAssignment` |
| STEMS outer B-linker assignment (fast evidence) | `apply_native_stems_beam_vlink_outer_b_linker_transaction` is the eighteenth exact boundary, the first under the fast-evidence policy in `rust/PORTING.md`: it executes the caller seam in `BLinker.link` after `VLinker.link` returns true - one idempotent `setLinked(true)` on the outer B-linker (the same shared cell Boundary 15 wrote from inside `VLinker.link`), plus the certified lexical-parent identity and the EnumMap loop-resumption facts. The frozen fast corpus is chula and BachInvention5 (9 of the 30 real transactions), generated by a single fresh-JVM pass whose runner required the re-emitted Boundary-17 rows to match the frozen head-links fixture byte-for-byte; the Rust gate re-pins that fixture by SHA-256, replays Boundaries 12-17 through the production functions, and matches every row field. All nine transactions are idempotent single-V writes with zero value changes; multi-V ordering, skip counting, and every refusal path are unit-covered. It stops at `AssignedOuterBLinkerBeforeNextVIteration`. The checkpoint has since raised it to full evidence: all eight sheets, two byte-identical fresh-JVM passes each, and the gate asserts it grades every installed sheet |
| STEMS scheduler SIDES resume (fast evidence) | `resume_native_stems_beam_scheduler_after_transaction` is the nineteenth exact boundary: after Boundary 18, the SIDES worklist resumes from the suspended position with exact Java loop semantics - remaining V linkers of the completed B, the side result the outer assignment determined (`SideBLinkerResult` with `linked_flag_after` true), remaining sides and beams - and stops at the second `ReadyForCreateStem` frontier as a typed `AwaitingVLinkTransaction`, or reports SIDES exhaustion. Scope is SIDES only; the STUMPS continuation is a later boundary. On chula and BachInvention5 all nine systems reach a RIGHT-side second frontier on the same beam, and every resumed expand outcome equals the frozen Boundary-10 matrix even though Java ran it against the post-transaction SIG - an equivalence the gate checks per row, so a page where the mutation changes an outcome fails loudly. The probe re-emits Boundary-17 and Boundary-18 rows and the runner requires byte-identity with their frozen fixtures. The checkpoint has since raised it to full evidence: all eight sheets under two byte-identical passes, with the gate asserting it grades every installed sheet. Those paths are now covered from real products rather than fabricated systems: chaining the corpus reaches SIDES exhaustion on most systems and competing-hook removal on allegretto 1 and 3, and the multi-V continuation occurs three times across two systems (batuque 1, BachInvention5 6) - multi-V B linkers are common but are almost never the frontier's own side linker, so that assertion is a deliberate floor of one and a failure there means the corpus stopped covering it. A failed link and an already-linked B linker are driven by re-running a real frontier with the one bit flipped, which shortens chula system 1 from 53 chained transactions to 36 |
| STEMS SIDES-pass chain (measured, not yet a boundary) | The pass is not a handful of transactions: a census probe run to exhaustion measures 31/29/33 fresh transactions per system on chula, so one boundary per transaction is not a viable shape and the port must iterate. `resume_native_stems_beam_scheduler_after_transaction` now composes - it returns an `advanced_system` carrying its events, deltas, `consumed_v_linkers` and `linked_b_linkers`, honours Java's `isLinked()` early return, and accepts a failed link. Driving that chain and comparing against Java exposes the remaining gap precisely: chula system 1 runs 53 chained transactions against Java's 32, the first 32 plan ordinals agree exactly, and the 21-transaction excess equals the 21 sides Java skips through the already-linked guard - whose B aliases have zero overlap with the executed ones because `linkSiblings` wrote their cells. Closing it means feeding `sibling_linked_b_linkers` from each transaction's Boundary-16 result, which needs the typed apply chain per transaction. `scheduler_resume_chain_composes_without_repeating_a_v_linker` proves composition only and says so |
| STEMS self-driving chain: what remains | Surveyed rather than assumed. A chain that carries its own state is blocked by **8** evidence guards, of which **7 are registry-completeness and none is genuinely per-transaction** - so the architecture is viable. Two registries start empty at STEMS and become authoritative by tracking alone: `systemStems` (`systemStems 0` in every baseline; the mechanism is now `NativeStemsBeamRegistryAuthority`, though production hydration still defaults to `RequiresExhaustiveScan`) and the HeadStem relation set. Four blockers share one bootstrap: the GlyphIndex (1,650 active on chula) and the beam-side SIG incident sets (639 vertices / 538 edges) start non-empty, so a chain must be told once per page what is already there. `AUDIVERIS_GLYPH_REGISTRY_OUT` on the Boundary-20 probe dumps that registry at the **first transaction's own baseline** - not at page load, where it is 1,253 rather than 1,650, because linker construction registers hundreds more in between. Java and Rust compute the run-table digest identically, so `g:x:y:w:h:<sha>` joins them. Two caveats to settle before relying on tracking: the hit-path glyph guard asks whether Java's **weak** index still retains a rejected compound's original, which would be the sole per-transaction blocker if it proves GC-dependent; and the SIG certificates encode JGraphT **insertion order**, so the bootstrap must be an ordered snapshot, not a set |
| STEMS chain self-drive: proven on the second transaction | The chain now runs a transaction on its own carried state and reaches Java's answer. With the page glyph registry bootstrapped once and joined by content digest, the line state seeded from the plan matrix, and `systemStems` declared `CompleteSinceEmptyBaseline`, chula's second create-stem produces plan 152 / ReuseActive / CreatedChecked - identical to the frozen Java evidence, which the gate reads only to check the answer and never to feed the transaction. `second_transaction_from_carried_state_reproduces_java` asserts that equivalence and runs by default in about six seconds. The remaining work to chain a whole SIDES pass is the SIG-side bootstrap (blockers 4-6 of the inventory: beam-side incident sets, which must be captured in JGraphT insertion order) and feeding each transaction's Boundary-16 sibling B-cell writes into the resume |
| What finishing STEMS actually needs, measured 2026-08-12 | Scoped before starting, and it is larger than the boundary backlog suggests. **There is no `recognize_native_stems`.** Every other ported stage has a production entry point (`recognize_native_heads`, `_ledgers`, `_beams`, `_stem_seeds`); STEMS exists only as boundary parity machinery driven by Java evidence. **The port owns no SIG.** `NativeStemsBeamSigApplyState` models the graph as an opaque baseline -- a vertex count, a relation count and two provenance hashes -- plus appends, which is exactly why base apply, head links and sibling links must *read* their incident scans from a Java certificate instead of computing them. `HeadlessGridSigState` is GRID-local and is not carried forward. **And this is by design**: HEADS deliberately emits identity-free heads 'without fabricating Java SIG or glyph IDs'. That choice is what now blocks self-drive, because every SIG operation STEMS performs is about identities and insertion order. So the path to a headless STEMS runs through the port owning a SIG with its own identities, accumulated across GRID..HEADS -- cross-stage infrastructure, not a STEMS detail. The stems-beam-sig-snapshot bootstrap supplies that baseline for *tests*; production needs the port to build it |
| STUMPS: the two open items are one item | `StumpSkippedAlreadyLinkedB` is declared but never constructed, and the reason is structural rather than an oversight. The STUMPS loop lives in `replay_prefix`, which reaches it only when the SIDES pass produced no frontier at all -- so no transaction has run, nothing is linked, and the guard cannot fire. It becomes reachable only once `resume_native_stems_beam_scheduler_after_transaction` grows a Stumps arm and carries `linked_b_linkers` in. Adding the guard alone would be dead code; the two items in the STUMPS scoping note are therefore a single change. Java's guard is `if (!vLinker.isLinked())` in `BeamLinker.linkStumps`, and `linked` is declared on **BLinker** with `VLinker` reading it through the enclosing instance -- so the port's `...AlreadyLinkedB` name is precise, and the V/B asymmetry in `VLinker.setLinked` delegating to `BLinker.this` is consistent, not a defect |
| STEMS Boundary 16 computes the sibling writes itself | The chain stops where Java stops when *fed* the writes; this shows the port can produce them rather than read them. `boundary_sixteen_derives_the_sibling_writes_the_pass_recorded` runs the production Boundary-16 apply on chula system 1's first transaction and its `sibling_b_linker_cells` are exactly the aliases the full-pass probe recorded -- `beam:0:b:0` and `beam:1:b:0`, the two that looked unexplained while the probe was still missing the first transaction's writes. Replay-on-frozen per rust/PORTING.md: proven on the one transaction with frozen evidence before being trusted on the 31 without. **What remains for a self-driving pass is now only carriage, not capability**: running the typed b12-b16 chain per transaction from carried state so transactions 2..32 produce their own cells, which is the same machinery `second_transaction_from_carried_state_reproduces_java` already runs for one transaction through b15 |
| STEMS chain: fed Java's sibling writes, it stops where Java stops | The 53-vs-32 gap is closed. `the_chain_fed_javas_sibling_writes_stops_where_java_stops` drives the resume chain on chula system 1 feeding each transaction the `linkSiblings` writes the frozen pass recorded, and it runs **exactly 32 transactions in Java's order** -- not merely the same count, the same B linkers in the same sequence. The join is Java's own alias `beam:<sigOrdinal>:b:<ordinal>`: the beam is found by SIG ordinal in the stump product and the B ordinal is Java's zero-based `allBLinkers` index against the port's one-based `id`, so the conversion is explicit and asserted rather than assumed. **Wiring it up exposed a real port defect**: `resume_native_stems_beam_scheduler_after_transaction` folded the completed transaction's linked B linkers into the state only *after* walking forward, so the walk could re-run a side that transaction had just linked. Java's `linkSiblings` writes those cells during the transaction, so the next side in the caller loop already sees them through `BLinker.link`'s `isLinked()`. On chula system 1 it cost exactly one extra transaction at the very end, where the 32nd transaction's sibling write is what retires the final side. Fixed by computing the updated set before the walk; the corpus and resume gates against frozen Java evidence still pass, which is what says the new ordering is Java's and not just convenient. What remains is computing those writes from the typed Boundary-16 applies instead of reading them from the oracle |
| STEMS SIDES pass: the skip model closes exactly | Measuring Java's whole pass rather than its frontier settles what a self-driving chain is missing. `StemsBeamSidesLoopProbe` now records, per transaction, which *other* B linkers `linkSiblings` left linked, plus the set already linked before the pass. On chula system 1: **32 transactions execute, 21 sides are skipped through `BLinker.link`'s `isLinked()` early return, and all 21 were linked by an earlier transaction's `linkSiblings`** -- with zero overlap between executed and sibling-linked B linkers. 32 + 21 = 53, exactly what the synthetic chain runs when it skips nothing, so the gap is fully accounted. **Nothing is linked before the pass begins**, so this is entirely tracked state and needs no bootstrap -- unlike the SIG and the GlyphIndex. That last point cost two wrong intermediate answers: sampling the baseline at the first *counted* transaction reported 3 already-linked linkers, and moving it one earlier still reported 2, because the probe's fresh-transaction counter does not count the first transaction and its sibling writes were therefore never recorded. Both figures were instrumentation artifacts; the true baseline is empty and the handoff's original account was right. `the_sides_pass_accounts_for_every_skipped_side` asserts the closure, so a regenerated fixture that breaks it fails loudly. Frozen for chula system 1 under two byte-identical fresh JVM passes; the remaining systems come with the boundary that consumes it |
| STEMS SIG bootstrap: one ordered snapshot is enough | The largest of the eight self-drive blockers, shared by base apply, head links and sibling links, all three of which refuse to run without a certificate whose incident scans are Java's `edgesOf`/`incomingEdgesOf`/`outgoingEdgesOf` in order. Unlike `systemStems` the SIG is non-empty when STEMS begins, so tracking alone cannot make a chain authoritative -- it must be told once what is already there. `AUDIVERIS_SIG_SNAPSHOT_OUT` on the Boundary-20 probe now dumps one system's SIG in JGraphT insertion order at its first transaction baseline, checking as it goes that nothing was added between the baseline capture and the dump. **Per system this is 221 vertices / 202 edges on chula system 1; the 639/538 recorded earlier is the sheet-wide total over three systems, so the bootstrap is a third the size the survey implied.** `sig_snapshot_derives_the_incident_scans_java_recorded` rebuilds Java's ordered vertex and edge hashes from the file -- a snapshot that parsed cleanly but described a reordered or truncated graph fails there rather than producing plausible scans -- and then derives the incident scans and matches Java's own recorded rows, `scope real` only. **The result: a per-vertex adjacency dump is unnecessary.** A vertex's incident list is the global edge order filtered to that vertex, incoming before outgoing, which is what `ExhaustiveIncomingThenOutgoing` names; the transaction's own new edge appears at ordinal `len(baseline)`, confirming append-at-end. Two fresh JVM passes are byte-identical, which is the check that matters for a fixture whose entire content is an ordering. Frozen for chula system 1 only; the remaining systems and sheets come with the boundary that consumes it |
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

Each page therefore retains one separate system-1 `IsolatedSyntheticSig` block.
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

The next blocker is no longer discovering, reconstructing, or querying the baseline
ordering. STEMS must project these owned query rows into the B14/B16/B17 certificate
types while carrying each transaction's appended vertices and edges.

**Slice 3 (BEAMS, ordinals 43-110), measured:** 48 hooks/beams interleaved in detection
order (`HBHBHB...` -- the hook usually precedes its beam), then all 20 BeamGroupInters.
Edges, all internal: 48 Containment (each group contains its members: 31 beams, 17 hooks),
44 BeamBeamRelation (13 beam-beam, 12 hook-beam, 12 beam-hook, 7 hook-hook), 10 Exclusion
(hook vs beam). `recognize_native_beams` already produces beams, hooks and groups.

**Slices 4-5:** LEDGERS (8 vertices, no edges), HEADS (102 vertices, 58 internal edges)
append in the same way, each verified against its
ordinal range. HEADS is the one that needs new identity assignment, since it deliberately emits
identity-free products today.

**Then** the STEMS certificates can be computed rather than read, which unblocks self-drive, and
`recognize_native_stems` becomes writable.

## Next implementation slices

Commit each slice separately after the full verification block above.

1. Replace the Java-fed B14/B16/B17 SIG scans with queries over the new owned graph,
   preserving insertion order and every transaction's appended relation/state overlay.
2. Drive the chula-system-1 SIDES pass from native products only: carry B12 through B18,
   feed Boundary-16 sibling B-cell writes into Boundary-19 resume, and compare Java's
   32-transaction pass only after the driver returns.
3. Extend the same carried graph/state through STUMPS, competing-hook removal, and the
   remaining scheduler branches, then expose a real `recognize_native_stems` entry point.
4. Extend `.omr` typing only through bounded read-only views that preserve every
   unknown byte and distinguish absent, malformed, and undeclared members explicitly.
5. Migrate future stage snapshots onto `audiveris-testkit` incrementally; keep the
   current vector ordering stable while its key-aware diagnostics catch schema drift.
6. Add Tesseract data to the oracle manifest when its resolved runtime location is
   known; the bundled classifier, fonts, JDK metadata, and image fixtures are frozen.
7. Freeze or vendor the three parent-corpus SCALE pages before expecting `xtask vectors`
   to work in a standalone Audiveris clone; today those vectors deliberately resolve
   `../../data/synth/...` from this parent OMR checkout.
8. Port deeper semantic behavior in `OmrStep` order; stop comparison at the first
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
