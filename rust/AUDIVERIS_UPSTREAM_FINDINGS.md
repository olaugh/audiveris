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
  beam-origin `StemBuilder` boundary is unaffected; the next head-origin
  builder boundary grades this branch explicitly.

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
- **Rust parity policy:** the current constructor boundary does not execute
  `expand`; a later linking oracle must quantify corpus impact before deciding
  whether compatibility mode preserves the typo or follows an upstream fix.

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

## Reporting notes

Findings in test probes, Rust-only code, or oracle normalization are not listed
here unless they reveal a corresponding Java production issue. Historical
oddities that are demonstrably intentional should be moved out of “Confirmed
findings” rather than silently deleted, so the parity rationale remains
traceable.
