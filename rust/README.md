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
```

Passing the Rust tests means only that the currently ported surface is compatible.
It does **not** yet mean that the whole Audiveris application or its recognition
accuracy has reached parity. See [PORTING.md](PORTING.md) for the gates and status.
