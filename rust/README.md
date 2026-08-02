# Audiveris Rust port

This directory is an incremental Rust port of Audiveris. It is a derived work of
Audiveris and is distributed under `AGPL-3.0-or-later`; see the repository-level
`LICENSE` and retain the original source attribution when translating code.

The Java application remains the behavioral oracle while the port is incomplete.
The first target is the headless recognition and export pipeline, not the Swing UI.

```sh
cargo test --workspace
cargo run -p audiveris-cli -- --help
cargo run -p xtask -- baseline --run-java
cargo run -p xtask -- vectors
cargo run -p xtask -- manifest
```

`xtask vectors` compiles a probe against the frozen production Java classes and
compares 37 canonical utility, geometry, assignment, image, run-table, section, and
pipeline-order results with Rust. Geometry is canonicalized to a declared `1e-15`
decimal boundary; integer, byte raster, topology, string, and ordering fields remain
exact.

`xtask manifest` verifies the frozen classifier, music fonts, and canonical image
fixtures before differential work begins. It accepts `--java-root PATH` like the
other oracle commands.

Passing the Rust tests means only that the currently ported surface is compatible.
It does **not** yet mean that the whole Audiveris application or its recognition
accuracy has reached parity. See [PORTING.md](PORTING.md) for the gates and status,
and [HANDOFF.md](HANDOFF.md) for an exact continuation checklist.
