# Running the Rust port on a Linux VM

Verified 2026-08-04 in a Claude Code remote container (Ubuntu, amd64) at tip
`8418c6a` on `codex/rust-port`. Read `PORTING.md` and `HANDOFF.md` first; this
file only covers what differs from the maxim.local (macOS) environment those
documents assume.

## Green results to expect

At `8418c6a`, a correctly provisioned Linux VM reproduces:

- `cargo test --workspace`: **878 passed, 0 failed**
- `cargo fmt --all --check`: clean; `cargo clippy --workspace --all-targets`: clean
- `cargo run -p xtask -- vectors`: **`Java/Rust parity: 74 canonical vectors match`**
- `cargo run -p xtask -- baseline --run-java`: **39 suites, 212 tests, 0
  failures, 0 errors, 1 skipped** (must match `rust/oracle/baseline.json`)

If any of these differ, stop and diagnose before building on the tip.

## 1. Getting the repo

The fork is public: `git clone https://github.com/olaugh/audiveris.git` works
unauthenticated (in a Claude Code container, prefer `add_repo` first so pushes
are possible; the plain clone is read-only). The Rust workspace is `rust/`;
run all cargo commands from that directory.

## 2. Rust toolchain

Any recent stable toolchain works (`rust-version = 1.85` in the workspace;
verified with cargo 1.94). Nothing else to install. First full
`cargo test --workspace` build takes a few minutes; reruns are fast.

Gotchas:

- `cargo test --workspace --quiet` hides per-crate summaries; drop `--quiet`
  and grep `^test result` to count.
- Concurrent cargo invocations in one checkout serialize on the target-dir
  lock; don't run a background `xtask vectors` and foreground builds together
  and expect both to make progress.

## 3. JDK 25 (required for anything touching the Java oracle)

`gradle.properties` sets `theMinJavaVersion = 25`. There is **no Gradle
toolchain auto-provisioning** (no foojay resolver), and Ubuntu images
typically ship JDK 21, which fails the build. Also, `xtask`'s default
`JAVA_HOME` fallback is a **macOS path** (`../jdk25/Contents/Home`), so on
Linux you must set `JAVA_HOME` explicitly.

Fetch Temurin 25 (works through the container's HTTPS proxy; ~140 MB):

```sh
curl -sL -o /tmp/jdk25.tar.gz \
  "https://api.adoptium.net/v3/binary/latest/25/ga/linux/x64/jdk/hotspot/normal/eclipse"
tar xzf /tmp/jdk25.tar.gz -C /opt   # yields /opt/jdk-25.0.x+y
export JAVA_HOME=/opt/jdk-25.0.4+7  # adjust to the extracted version
```

Do not unset the preconfigured `JAVA_TOOL_OPTIONS` (proxy + truststore); the
Gradle distribution and dependency downloads rely on it. The first
oracle-touching command compiles the full Java app (~327k lines) — expect
5–15 minutes; later runs reuse the Gradle caches.

## 4. Out-of-repo scale fixtures (the trap that bites first)

Both the Rust vectors and the Java `RustParityProbe` load three synthetic
pages from **outside the repository**, resolved relative to the repo root:

```
../../data/synth/k545-movement1-exposition/page-001.png
../../data/synth/essenfolksong-erk20/page-001.png
../../data/synth/josquin-4vperilludaveprolatum/page-001.png
```

These exist only on maxim.local and are not pinned in
`rust/oracle/manifest.sha256`. On any other machine, `xtask vectors` (even
`--rust-only`) fails on the first one. As of the takeover commit the error
names the missing path; before it, the symptom was a bare
`error: No such file or directory (os error 2)`.

Two options:

- **Copy the real fixtures** from maxim.local if you can — then the
  `scale.k545` / `scale.essen` / `scale.josquin` vector values match that
  machine's records exactly.
- **Substitute any three distinct grayscale music pages** (e.g. from
  `data/examples/`). Parity remains a valid test — both runtimes read the
  same bytes — but those vectors' *values* will differ from maxim.local's.
  Say so in any checkpoint record.

```sh
# from the repo root; ../../ resolves two levels above the checkout
mkdir -p ../../data/synth/k545-movement1-exposition \
         ../../data/synth/essenfolksong-erk20 \
         ../../data/synth/josquin-4vperilludaveprolatum
cp data/examples/allegretto.png ../../data/synth/k545-movement1-exposition/page-001.png
cp data/examples/batuque.png    ../../data/synth/essenfolksong-erk20/page-001.png
cp data/examples/carmen.png     ../../data/synth/josquin-4vperilludaveprolatum/page-001.png
```

Open follow-up for whoever touches the harness next: pin these fixtures by
SHA-256 in `rust/oracle/manifest.sha256` (or relocate them under
`rust/oracle/`) so the parity suite becomes machine-independent.

## 5. Command sequence for a full verification pass

```sh
cd rust
cargo test --workspace                       # expect 878 passed
cargo fmt --all --check
cargo clippy --workspace --all-targets
JAVA_HOME=/opt/jdk-25.0.4+7 cargo run -p xtask -- vectors           # 74/74
JAVA_HOME=/opt/jdk-25.0.4+7 cargo run -p xtask -- baseline --run-java  # 212 tests
```

`cargo run -p xtask -- vectors --rust-only` prints the Rust side without
invoking Gradle — useful for a fast smoke check that the fixtures resolve.

## 6. Container-specific notes

- Outbound HTTPS must go through the preconfigured agent proxy; Adoptium,
  Gradle services, and crates.io all work through it. Never disable TLS
  verification.
- If `add_repo` approval is unavailable, the public clone is read-only:
  commit locally on a clearly named branch and hand the diff back in the
  session transcript, as was done for the fixture-error-context fix on
  `claude/rust-port-takeover`.
- Disk: the Gradle caches plus a JDK plus the cargo target dir total several
  GB; on "no space left on device", delete `rust/target/` and Gradle caches
  first — the per-session allowance, not the disk, is what's exhausted.
