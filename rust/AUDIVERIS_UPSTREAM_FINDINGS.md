# Audiveris upstream findings

This catalog records issues found while porting Audiveris to Rust. It is meant
to be useful to the Java maintainer: each entry distinguishes observed Java
behavior from the Rust compatibility policy. Exact line numbers refer to the
Java source checked into this repository and may move.

## Confirmed findings

### AV-JAVA-001: `filterHeadParts` removes small remnants only for VIP heads

- **Location:**
  `app/src/main/java/org/audiveris/omr/sheet/stem/StemBuilder.java`,
  `filterHeadParts`, around lines 374–386.
- **Observed behavior:** when a chunk overlapping the head has fewer than 15
  pixels remaining after head pixels are discounted, `Iterator.remove()` is
  nested inside `if (cLinker.getHead().isVip())`. A non-VIP head therefore
  retains the chunk. The preceding VIP block is empty.
- **Why this looks unintended:** VIP is otherwise a diagnostic/logging flag;
  it should not change recognition semantics. The method documentation says it
  filters chunks by remaining weight without mentioning a VIP exception.
- **Likely repair:** keep the log statement conditional on VIP, but move
  `it.remove()` outside the inner VIP condition while retaining the
  `remain < 15` guard.
- **Rust parity policy:** preserve the Java behavior until the upstream Java
  behavior changes and a new oracle fixture is frozen. The completed
  head-origin `StemBuilder` boundary grades this branch explicitly and keeps all
  6,087 low-remain non-VIP corpus chunks.

### AV-JAVA-002: `StemItem.lineOf` incorrectly says the `BLinker` branch covers `VLinker`

- **Location:**
  `app/src/main/java/org/audiveris/omr/sheet/stem/StemItem.java`, around line
  275; the relevant hierarchy is in `BeamLinker.java`, where `VLinker extends
  StemHalfLinker`.
- **Observed behavior:** the comment on `if (linker instanceof BLinker)` says
  “Applies to VLinker as well,” but a `VLinker` is not a `BLinker`. A
  stump-less VLinker therefore falls through and receives a degenerate line at
  its reference point. Only an actual stump-less BLinker receives the short
  beam-height segment.
- **Impact:** the misleading comment caused an independent port review and an
  early oracle replay to infer the wrong start-item geometry. Reflection over
  real Java `StemBuilder` items confirmed the degenerate VLinker line.
- **Likely repair:** correct the comment if the degenerate VLinker line is
  intended. If the segment was intended for VLinker too, the implementation
  needs an explicit VLinker case rather than the current `instanceof BLinker`
  test.
- **Rust parity policy:** reproduce the actual degenerate VLinker line.

### AV-JAVA-003: `CLinker.expand` looks up `i` inside its `j` look-ahead loop

- **Location:**
  `app/src/main/java/org/audiveris/omr/sheet/stem/HeadLinker.java`, lines
  1182–1201, in `CLinker.expand`.
- **Observed behavior:** after encountering a beam item at index `i`, the code
  loops over `j = i + 1 .. maxIndex` to look for another reachable beam, but
  assigns `ev2 = sb.get(i)` instead of `sb.get(j)`. If any later item exists,
  the first loop iteration therefore re-reads the current beam, finds the same
  beam group, sets `stop = false`, and exits the look-ahead.
- **Impact:** every non-final beam item is treated as though a later beam from
  the same group were reachable. Expansion can continue past the beam and
  inspect/add later stem material or relations even when no later same-group
  beam exists. If the beam is the final item, the loop is empty and stopping
  still occurs.
- **Minimal regression shape:** a builder item list containing a beam from
  group A followed only by a non-beam item. The intended look-ahead finds no
  later beam and stops at the beam; the current code rechecks the group-A beam
  itself and continues.
- **Likely repair:** change `sb.get(i)` to `sb.get(j)` and freeze a linking test
  covering both “no later beam” and “later same-group beam” cases.
- **Rust parity policy:** the completed beam-origin expansion boundary does not
  execute this separate head-origin `CLinker.expand`; a later head-linking oracle
  must quantify corpus impact before deciding whether compatibility mode
  preserves the typo or follows an upstream fix.

### AV-JAVA-004: downward beam expansion mutates its stored theoretical line

- **Locations:**
  `app/src/main/java/org/audiveris/omr/sheet/stem/BeamLinker.java`,
  `VLinker.buildLuArea` and `VLinker.expand`, around lines 1148 and 1185; and
  `app/src/main/java/org/audiveris/omr/sheet/stem/StemHalfLinker.java`,
  `updateStemLine`, around lines 72–103.
- **Observed behavior:** for a downward VLinker, `expand` assigns
  `stemLine = theoLine` rather than making a copy. `updateStemLine` then calls
  `setLine` on that object for each new structural Glyph. The same object is
  retained by the VLinker, by its `StemBuilder`, and, when that V is the current
  value for the BLinker's attachment key, by the beam's `theo-<id>` diagnostic
  attachment. Upward expansion reverses the endpoints into a new `Line2D` and
  therefore does not mutate the stored line.
- **Live evidence:** the isolated eight-page expansion matrix observes 3,226
  downward stored-line mutations, all mirrored by the current beam attachment.
  The largest horizontal shift is about 8.48613 pixels on Chula. The probe
  restores the exact pre-call bits between matrix variants; normal Java linking
  does not perform that restoration.
- **Impact:** an ostensibly local feasibility pass changes later geometry and
  diagnostic state. Its result is path-dependent if the same VLinker is tried
  again or inspected after a failed attempt, and a visual attachment can move as
  a side effect of recognition rather than merely displaying the originally
  constructed lookup geometry.
- **Likely repair:** make the working line an explicit copy in both directions,
  then commit an accepted line deliberately if persistent adjustment is wanted.
  If persistence is intentional, document it and test retry/failure ordering.
- **Rust parity policy:** the isolated planner remains immutable but emits the
  exact stored-line and attachment delta. A later serial scheduler must apply
  that delta in Java order until upstream behavior is deliberately changed and
  re-frozen.

### AV-JAVA-005: `BEAM_SIDE` expansion does not enforce its documented terminal head

- **Location:**
  `app/src/main/java/org/audiveris/omr/sheet/stem/BeamLinker.java`,
  `VLinker.expand` and `VLinker.link`, around lines 1159–1271 and 1576–1615.
- **Observed behavior:** the `expand` javadoc says a `Profiles.BEAM_SIDE` stem
  must end at a head on the correct horizontal side. The loop tracks such a
  stopping head, but ordinary exhaustion returns `maxIndex` unconditionally.
  `link` checks only the returned index, nonempty relations, and nonempty Glyphs;
  it never requires or rewinds to the tracked stopping head.
- **Live evidence:** among 1,286 ready profile-4 rows in the eight-page matrix,
  9 have no valid stopping head at all, 632 have one but return beyond it, and
  only 645 return at the last valid stopping head.
- **Impact:** a beam-side stem can be accepted despite violating the method's
  stated terminal invariant, or can include material beyond the last head that
  satisfies it. A caller cannot infer the documented condition from a
  successful `link` prefix.
- **Likely repair:** on profile 4, fail if no acceptable stopping head exists and
  otherwise return the stopping snapshot rather than `maxIndex`; add tests for
  no-stop, trailing-material, and exact-stop cases. Confirm the desired relation
  rollback at the same time.
- **Rust parity policy:** preserve the observed three-way result partition in
  compatibility mode. Do not silently enforce the javadoc until Java changes
  and a new oracle is frozen.

## Risks under audit

### AV-JAVA-RISK-001: `StemBuilder.sortItems` uses a pair-dependent comparator

- **Location:**
  `app/src/main/java/org/audiveris/omr/sheet/stem/StemBuilder.java`,
  `sortItems`.
- **Behavior:** two `HalfLinkerItem`s compare by linker reference-point Y, but
  every mixed pair compares by line endpoints. This pair-dependent comparator
  is not transitive and therefore violates the sorting contract.
- **Live evidence:** the corrected exhaustive Chula beam-origin audit finds 2
  strict comparator cycles (including one four-item final list) and 81
  equivalence inconsistencies: 22 in target-only sorts and 59 in final-item
  sorts. Across the full eight-page corpus, the totals are 18 strict cycles
  and 2,503 equivalence inconsistencies. The oracle retains the exact
  input/output permutations plus hashes identifying offending triples.
- **Impact:** the result can depend on the sorting implementation and may fail
  or change when Java's TimSort implementation changes. A generic stable Rust
  sort is not a valid compatibility substitute even though most inputs happen
  to order identically.
- **Likely repair:** define one context-independent vertical key for every item
  kind, or otherwise make the comparator a total preorder before sorting.
- **Rust parity policy:** reproduce the frozen OpenJDK ordering at this boundary
  and keep the anomaly evidence visible; do not normalize the comparator or
  silently rely on Rust's standard stable sort.

### AV-JAVA-RISK-002: rejected head stumps remain in the sheet glyph index

- **Location:**
  `app/src/main/java/org/audiveris/omr/sheet/stem/HeadLinker.java`, around lines
  870–879.
- **Behavior:** `buildStump` calls `GlyphIndex.registerOriginal` before
  `standsOut`. When `standsOut` rejects the candidate and the CLinker retains
  no stump, the registered glyph still remains available as a sheet-global
  canonical object.
- **Impact:** on Chula, eight later `StemBuilder` chunk-registration attempts
  reuse glyphs left by rejected, unattached head-stump candidates. Across the
  eight-page corpus, 142 chunk-registration attempts have rejected-head-stump
  content. A consumer reconstructing the glyph index only from attached
  CLinker stumps silently gets different New/Reuse identities.
- **Status:** this may be intentional canonicalization rather than a defect,
  but it is an observable, order-dependent side effect worth confirming with
  the maintainer.
- **Rust parity policy:** stage every head-stump registration attempt,
  including rejected candidates, before the current system's beam-origin
  builders.

### AV-JAVA-RISK-003: a VLinker direction can disagree with its StemBuilder direction

- **Locations:**
  `app/src/main/java/org/audiveris/omr/sheet/stem/BeamLinker.java`, around
  lines 1054 and 1147, and
  `app/src/main/java/org/audiveris/omr/sheet/stem/StemBuilder.java`, around
  lines 156–160.
- **Behavior:** `VLinker` retains the requested vertical direction, but
  `StemBuilder` does not consume that field. It independently derives its
  direction from `theoLine.getY2() > theoLine.getY1()`. A closer-beam limit can
  put the final theoretical-line endpoint on the opposite side of the
  reference point, so the two directions are not guaranteed to agree.
- **Live evidence:** the exhaustive eight-page beam-origin audit finds one
  disagreement in 2,417 builders: `carmen.png`, system 2, builder 56 has
  VLinker direction down (`+1`) and StemBuilder direction up (`-1`). All five
  Java length-map entries for profiles 0 through 4 are zero. Treating the
  VLinker direction as the builder direction instead produces length 1 in
  every row.
- **Impact:** callers and diagnostic tools can silently use the wrong
  direction if they assume the VLinker direction is inherited by its builder.
  The duplicate representation also makes later filtering, sorting, gap, and
  length behavior depend on which direction a reimplementation chooses.
- **Status:** this may be a tolerated degenerate geometry rather than an
  intended invariant. It is worth either asserting that the two directions
  agree when geometry is built or documenting why the builder is allowed to
  reverse direction.
- **Rust parity policy:** retain both values as explicit evidence and use the
  direction derived from the final theoretical line for every StemBuilder
  operation.

### AV-JAVA-RISK-004: compressed MusicXML exports are not byte-reproducible

- **Location:** the Java MusicXML export path that creates `.mxl` ZIP
  containers.
- **Behavior:** two otherwise identical Chula exports contain byte-identical
  MusicXML but have different archive SHA-256 values because ZIP entry
  timestamps vary between runs.
- **Live evidence:** the inner `chula.xml` and the direct uncompressed export
  are identical at 65,354 bytes with SHA-256
  `317acfee5e54d73a82f97f2a44a6b640e59ad6062b127ceee088420b94d6fa2c`,
  while repeated `.mxl` container hashes differ.
- **Impact:** an archive hash is useful for checking one artifact after it is
  handed between processes, but not as a semantic cache key or deterministic
  regression pin.
- **Likely repair:** set deterministic ZIP metadata, including entry
  timestamps, when reproducible exports are desired.
- **Rust parity policy:** validate and hash each produced artifact for local
  integrity, but compare normalized inner MusicXML when judging repeatability.

### AV-JAVA-RISK-005: `Sheet.export(Path)` does not report export failure to callers

- **Location:**
  `app/src/main/java/org/audiveris/omr/sheet/Sheet.java`, around lines 737–779.
- **Behavior:** `Sheet.export(Path)` catches export exceptions, logs them, and
  returns `void`. A programmatic caller cannot distinguish a successful export
  from a failed one through the method result or an exception.
- **Impact:** automation can proceed with a missing, stale, or partial output
  unless it separately validates the requested artifact. This surfaced while
  adding the Java PAGE-to-MusicXML preview to `omrscope`.
- **Likely repair:** propagate the exception or return an explicit result while
  retaining any desired user-facing log message at the UI boundary.
- **Rust parity policy:** the export probe reopens the exact requested path and
  validates its format, size, and digest before reporting success.

### AV-JAVA-RISK-006: beam expansion rewind is only a partial rollback

- **Location:**
  `app/src/main/java/org/audiveris/omr/sheet/stem/BeamLinker.java`,
  `VLinker.expand`, around lines 1194–1231.
- **Behavior:** when a later gap or separated head rewinds to the most recent
  valid stopping head, Java restores the saved Glyph set and returned item
  index. It does not restore the working stem line and does not remove relation
  entries accumulated after that snapshot.
- **Live evidence:** the frozen corpus currently has no relation whose item lies
  beyond the returned index, but 49 gap rewinds retain a bit-different working
  line after Glyph restoration. The maximum coordinate residual is
  `0x1.0p-39` (about 1.82e-12 pixels).
- **Impact:** returned Glyphs, relations, and geometry are not guaranteed to
  describe one common prefix. The present corpus makes the geometric difference
  tiny, but another input can expose a larger line or relation inconsistency.
- **Likely repair:** snapshot and restore all three outputs together, or make the
  asymmetric contract explicit and cover a case with a relation after the
  stopping point.
- **Rust parity policy:** retain the asymmetric rollback and grade the divergence
  explicitly; do not reconstruct the final line solely from returned Glyphs.

### AV-JAVA-RISK-007: a missing leading barline costs the whole grand staff

- **Locations:**
  `app/src/main/java/org/audiveris/omr/sheet/grid/PeakGraph.java`,
  `getSystemTops` around lines 1040-1053 and `areRightConnected` around lines
  144-159; `app/src/main/java/org/audiveris/omr/sheet/SystemManager.java`,
  `minIndentation`.
- **Behavior:** the first connection that would assign a bottom staff to a
  system is discarded when it starts more than `maxFirstConnectionXOffset`
  (default 2 interlines) right of that staff's left abscissa, unless the *last*
  peaks of both staves are themselves connected. `systemTops[bottom - 1]` stays
  null on rejection, so later connections are re-tested by the same rule; since
  connections are ordered by abscissa, once the leftmost fails the rest are
  further right and fail identically.
- **Impact:** a leading barline too faint to detect therefore costs the entire
  grand staff, and each staff becomes its own system. Braces are then found only
  for systems whose leading barline was detected -- elsewhere
  `getStartPeakIndex()` returns -1 and the staff logs as a one-staff system --
  so a piano page exports as three parts rather than one. Residual horizontal
  foreshortening in the same photographed input independently trips
  `minIndentation` (also 2 interlines), and `allocatePages` reports phantom
  movements. Measured on a phone photo of a piano score: connections existed at
  grades up to 0.76 for exactly the right staff pairs and were all discarded;
  raising both constants took the page from 11 measures across 3 phantom
  movements to 43 measures in one. A control PDF of the same music ruled out
  resolution (interline 19 against the photo's 20).
- **Status:** *not a defect.* `getSystemTops` carries an explicit
  `TODO: What if very first connection is missing but we have more on right?`
  and `areRightConnected` is the implemented second chance, so the case is
  known. What is worth raising with the maintainer is that the escape requires
  the last peaks of both staves to be connected, which fails on exactly the
  inputs that lose the leading barline -- a faint or cropped final barline
  defeats it -- and that the failure is silent: nothing in the output points at
  a threshold, so a user sees only phantom movements and split parts.
- **Rust parity policy:** none. Reproduce the Java thresholds and the
  short-circuit exactly; this entry records why a photographed page behaves the
  way it does, not a behaviour to diverge from.

## Reporting notes

Findings in test probes, Rust-only code, or oracle normalization are not listed
here unless they reveal a corresponding Java production issue. Historical
oddities that are demonstrably intentional should be moved out of “Confirmed
findings” rather than silently deleted, so the parity rationale remains
traceable.
