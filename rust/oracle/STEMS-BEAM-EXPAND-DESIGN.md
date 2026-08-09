# STEMS beam `VLinker.expand` oracle

## Boundary

This checkpoint freezes the source prefix of
`BeamLinker.BLinker.VLinker.link(stemProfile, linkProfile)` through the return of
private `VLinker.expand(...)`. It stops before `StemBuilder.createStem`, beam-stem
link checking, `SIGraph` writes, `systemStems` writes, linker state changes, and
all sibling/head link application.

The live Java checkpoint is the completion of `StemsRetriever.inspectStems()`:

1. reach real `HEADS` input state;
2. install the retriever parameters, purged system seeds, abscissa-sorted beams,
   and abscissa-sorted heads;
3. construct all beam and head linkers;
4. invoke every beam `inspectVLinkers()` in beam-x order;
5. invoke every head `inspectCLinkers()` in head-x order.

Constructing all C builders is required. The beam expand close-head branch calls
`clOpp.hasConcreteStart(linkProfile)`, which reads the opposite corner's completed
`StemBuilder`.

## Matrix

For each inspected, non-anchor beam V builder, the oracle evaluates
`stemProfile=0..constructionMax`, where the construction maximum is
`Profiles.BEAM_SEED` (3) for a center/stump B linker and `Profiles.BEAM_SIDE` (4)
for a side B linker. `linkProfile` is the effective system profile (1 in the
current corpus).

This is an isolated semantic matrix. It deliberately does not claim which row
the later live scheduler selects. Exact competing-hook selection requires live
Exclusion edges plus canonical Glyph identity, which the current immutable
native prerequisites do not expose.

The outcome precedence is source-faithful:

1. `NoHeadTarget` -- `sb.getCLinkers(null)` is empty, so `expand` is not invoked;
2. `ExpandFailed` -- `expand` returns -1;
3. `NoRelations` -- the ordered relation map is empty;
4. `NoGlyphs` -- the ordered glyph set is empty;
5. `ReadyForCreateStem` -- execution has reached the call site immediately before
   `StemBuilder.createStem`.

## Evidence

The probe invokes the real private `expand` reflectively and independently
replays its loop in exact item order. It compares last index, ordered relation
keys and complete `HeadStemRelation` payloads, and ordered retained Glyph
identities.

Emitted rows freeze:

- plan inputs: stable V/B reference, profiles, item/head counts, gap thresholds,
  `minLinkerLength`, and the oriented theoretical line;
- every gap decision, including the current dynamic gap threshold and rewind
  state;
- every close-head separation check, opposite corner, opposite-builder length,
  and concrete-start decision;
- every relation attempt: stored C corner versus dynamically derived head side,
  reference point, raw pixel gaps, scaled gaps, profile maxima, unclamped and
  clamped impacts, weights, grade/minimum, extension point, stopping portion,
  ordered-map insertion/replacement, and pre-current-C stopping snapshot;
- every glyph update: attempted item occurrence, the retained first
  content-equal insertion occurrence, ordered set before/after, composite
  bounds/weight/RunTable digest/centroid, intersection, x shift, and line before
  and after;
- final ordered glyph and relation rows plus result/rollback diagnostics.

`Glyph.equals` is content-based. The replay therefore tracks the actual insertion
occurrence for each retained Glyph identity; it does not label a result with the
first global item that merely references the same object.

## Source asymmetries pinned by the oracle

- A stopping-head Glyph snapshot is taken before the current C stump is added.
- Gap and separated-head rewind restore Glyphs and last index, but do not remove
  relations inserted after the stopping item and do not restore later local
  stem-line shifts. The fixture reports relations past the returned index and
  compares the retained final line with a line recomputed from the restored
  Glyph set.
- `HeadStemRelation.checkRelation` derives its horizontal head side from
  `-stemLine.relativeCCW(head.center)`; it need not equal the encountered C
  linker's stored horizontal corner side.
- The BEAM_SIDE javadoc says expansion must end at a correct-side head, but the
  method returns `maxIndex` unconditionally after exhausting items. The fixture
  counts ready profile-4 rows with no stopping update and ready profile-4 rows
  whose returned index is not the last source-faithful stopping index.
- For downward V linkers, Java aliases the local `stemLine` to the stored
  `VLinker.theoLine`. The V field, `StemBuilder.theoLine`, and (when current) the
  beam `theo-<B id>` attachment are the same mutable object. Glyph insertion thus
  persistently translates all three. Upward expansion reverses into a copy and
  does not mutate the stored line. The matrix records exact pre/post bits and
  shift, then restores the shared object's exact coordinates in a `finally`
  block so every profile variant starts from the same inspected checkpoint.

## Mutation assertions

Each plan snapshots and verifies SIG vertices/edges, stem inters, `systemStems`,
GlyphIndex identities, FilamentIndex identities, B/V/C linked and closed flags,
all V/C builder assignments, builder item identities/geometry/contributions, and
length maps. These remain unchanged. The downward shared-theoretical-line shift
is not mislabeled as zero mutation: it is explicitly graded and restored only to
isolate the matrix variants.

## Chula checkpoint

The first live checkpoint covers 3 systems, 354 beam builders, and 1,735 matrix
plans. It yields 625 `NoHeadTarget`, 100 `ExpandFailed`, 2 `NoRelations`, 14
`NoGlyphs`, and 994 `ReadyForCreateStem` outcomes, with 2,034 final relations and
1,478 final Glyph entries. `minLinkerLength` is 18 pixels for interline 21.

It contains 161 downward three-way theoretical-line/attachment mutations; the
maximum absolute horizontal translation is
`0x1.0f8e60e2a3f8p3` (raw bits `4020f8e60e2a3f80`). Two accepted/rejected relation
attempts derive a head side different from the stored C corner. Of 178 ready
profile-4 plans, 9 have no stopping update, 82 stop correctly and then return
beyond that stopping head, and 87 return exactly at their last valid stopping
head. Chula has no surviving relation past a rewound return and no rollback-line
divergence; the full eight-page freeze remains responsible for broader branch
coverage.

## Eight-page exploratory checkpoint

The complete corpus covers 30 systems, 2,417 builders, and 11,573 plans. Outcome
counts are 2,903 `NoHeadTarget`, 289 `ExpandFailed`, 2 `NoRelations`, 58
`NoGlyphs`, and 8,321 `ReadyForCreateStem`. It retains 18,345 final relations and
12,523 final Glyph entries.

The 578 gap rows split into 289 fail, 192 rewind, and 97 continue decisions; the
9,869 separation rows split into 9,867 continue and 2 rewind decisions. Of
18,416 relation attempts, 18,345 are accepted and 71 rejected. Every accepted
relation is a new ordered-map entry; no replacement occurs. Glyph updates split
into 12,582 insertions, 23,965 content-equal skips, and 1,136 null-Glyph calls.

The 194 rewind returns remove 59 post-snapshot Glyph insertions. No relation has
an item index beyond the returned index. Forty-nine gap rewinds nevertheless
retain a bit-different local line; their maximum coordinate residual is
`0x1.0p-39` (about 1.82e-12 pixels). This corpus manifestation is tiny, but the
source asymmetry remains part of the contract.

| Page | Plans | Ready | Gap fail / rewind / continue | Separation rewind | Line divergence | Shared-line mutation | Profile-4 ready: no stop / beyond / at | Max shared shift |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| chula | 1,735 | 994 | 100 / 0 / 25 | 0 | 0 | 161 | 9 / 82 / 87 | 8.48613018289302 |
| allegretto | 1,087 | 882 | 0 / 25 / 0 | 0 | 5 | 600 | 0 / 86 / 56 | 0.919648642772245 |
| batuque | 1,572 | 1,044 | 28 / 10 / 11 | 2 | 0 | 294 | 0 / 79 / 85 | 3.43538454890472 |
| carmen | 2,215 | 1,225 | 136 / 5 / 38 | 0 | 5 | 1,099 | 0 / 135 / 102 | 1.25531717884269 |
| cucaracha | 270 | 230 | 0 / 5 / 0 | 0 | 5 | 60 | 0 / 24 / 22 | 0.685534715810945 |
| hove | 881 | 436 | 0 / 35 / 0 | 0 | 10 | 436 | 0 / 28 / 28 | 1.02160259928132 |
| zizi | 610 | 430 | 16 / 0 / 4 | 0 | 0 | 240 | 0 / 23 / 63 | 0.763419045438695 |
| BachInvention5 | 3,203 | 3,080 | 9 / 112 / 19 | 0 | 24 | 336 | 0 / 175 / 202 | 3.88362333333072 |

Corpus totals are 3,226 downward shared-line mutations, always mirrored by the
current beam attachment alias, and two dynamic relation-side mismatches (both
rejected Chula attempts). The profile-4 ready census is 1,286: 9 without a
stopping head, 632 returning beyond the last stopping head, and 645 returning at
it. The maximum shared-line shift is Chula's
`0x1.0f8e60e2a3f8p3` (about 8.48613 pixels). All forbidden graph, index, linker,
and builder mutations remain zero.

## Split fixture lifecycle

The eight page streams are frozen separately. Within a plan, detail rows are
ordered `gap` / `separation` / `relation` / `update` by replay chronology,
followed by all ordered `glyph` rows, all ordered `finalrelation` rows, then the
single `end` row. A page file contains the common header, exactly one page
stream, then this trailer label order:

The one-page Chula runner uses Epsilon GC. Full exploration starts every page in
a fresh JVM with the JDK default collector because Bach's transient replay
evidence exhausts a 48 GiB Epsilon heap. Collector choice is not represented as
semantic evidence; both modes are byte-determinism checked.

`stemsbeamexpandcorpussummary schema ... mode ... pages ... pageRefs ...`
`rowCounts ... probeSourceSha256 ... runnerSourceSha256 ...`
`emittedBodySha256 ... emittedBodyLines ... emittedBodyBytes ...`

The manifest schema is `stems-beam-expand-manifest-v1`. Its entry row name is
`stemsbeamexpandmanifestentry`, with labels in this order: `ordinal`, `page`,
`fixture`, `pageHash`, `rowCounts`, `emittedBodySha256`, `emittedBodyLines`,
`emittedBodyBytes`, `fixtureSha256`, `fixtureLines`, `fixtureBytes`. Its final
row name is `stemsbeamexpandmanifestsummary`, with labels in this order:
`schema`, `entries`, `probeSourceSha256`, `runnerSourceSha256`,
`manifestBodySha256`, `manifestBodyLines`, `manifestBodyBytes`.
