# `audiveris-mei`

`audiveris-mei` is the serializer-only foundation for direct MEI output from
the Rust port. It emits deterministic MEI 5.1 CMN XML for a caller-supplied,
resolved score model:

- score/part/staff definitions, clefs, key signatures, and meters;
- measures and ordered layers;
- notes, rests, chords, beams, and barlines;
- explicit cross-staff targets and per-event Audiveris responsibility;
- stable event `xml:id` values derived from caller-owned semantic identities.

The crate validates the complete model before writing. It never reads the
clock, sorts musical source order, uses a process-order allocator, or infers
pitch/rhythm from geometry. Deterministic wrapper IDs derive from the supplied
sheet, measure, and staff identities; cross-run stability of caller IDs remains
a caller contract. A checked-in two-staff golden is byte-exact, independently
parsed, metrically checked, validated offline against the pinned MEI 5.1 CMN
Relax NG schema, and manually smoke-rendered locally with Verovio.

## Current boundary

There is intentionally no dependency on `audiveris-omr`. Native recognition
currently publishes through HEADS; its later MEASURES, CHORDS, RHYTHMS, and
PAGE contracts do not yet produce an ordered semantic score with resolved
pitch, duration, voices, and export-stable identities. Connecting those partial
products now would fabricate music. A thin adapter belongs here only after the
native PAGE product can hydrate the serializer model honestly.

Facsimile zones, confidence/impact annotations, control events, alternatives,
book-level movements, and omrscope integration are later additive phases.

Run the focused checks from `rust/`:

```sh
cargo test -p audiveris-mei --all-targets
cargo clippy -p audiveris-mei --all-targets -- -D warnings
scripts/validate-mei-schema.sh
```
