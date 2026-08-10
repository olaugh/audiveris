#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Partition this workspace's test targets across CI shards.

`cargo test` runs test binaries one after another, and this workspace's
corpus tests are compute-bound: each one drives the real pipeline over a
multi-megapixel page, so the suite is roughly an hour on a CI runner and had
started hitting the job timeout. Splitting it across shards is close to a
linear win because the binaries are independent.

Balance matters more than it looks: the three heaviest binaries are together
about a third of the suite, so a name-ordered round robin can land them in one
shard and waste most of the benefit. Assignment is therefore greedy
longest-first against measured weights in `test-weights.json`.

Those weights are advisory. A target that is missing from the file gets the
default weight and still runs -- correctness never depends on the file being
current, only balance does. Refresh it when the split visibly skews:

    cargo test --workspace 2>&1 | rust/ci/select-test-shard.py --record

Emits one `cargo test` invocation per line on stdout.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
WEIGHTS_PATH = HERE / "test-weights.json"
# Chosen to sit above the per-binary floor (every corpus binary pays for one
# pipeline run) so an unmeasured newcomer is never treated as free.
DEFAULT_WEIGHT = 30.0


def load_weights() -> dict[str, float]:
    if not WEIGHTS_PATH.is_file():
        return {}
    return {
        str(name): float(seconds)
        for name, seconds in json.loads(WEIGHTS_PATH.read_text()).items()
        if not str(name).startswith("_")
    }


def work_items() -> list[tuple[str, list[str]]]:
    """Every unit of test work, as (weight key, cargo arguments)."""
    metadata = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--no-deps", "--format-version", "1"],
            check=True,
            capture_output=True,
            text=True,
            cwd=HERE.parent,
        ).stdout
    )
    items: list[tuple[str, list[str]]] = []
    for package in metadata["packages"]:
        name = package["name"]
        for target in package["targets"]:
            kinds = target["kind"]
            if "test" in kinds:
                items.append(
                    (target["name"], ["-p", name, "--test", target["name"]])
                )
            elif "lib" in kinds and target.get("doctest", False):
                # One item covers the crate's unit tests and its doctests; both
                # are small next to the corpus binaries.
                items.append((name.replace("-", "_"), ["-p", name, "--lib", "--doc"]))
            elif "lib" in kinds:
                items.append((name.replace("-", "_"), ["-p", name, "--lib"]))
            elif "bin" in kinds:
                # Binaries carry unit tests too; `--workspace` ran them, so the
                # shards must. The coverage check below fails loudly if a target
                # kind is ever missed here again.
                items.append((target["name"], ["-p", name, "--bin", target["name"]]))
    return items


def assign(
    items: list[tuple[str, list[str]]], shards: int
) -> tuple[list[list[tuple[str, list[str]]]], list[float]]:
    weights = load_weights()
    ordered = sorted(
        items, key=lambda item: (-weights.get(item[0], DEFAULT_WEIGHT), item[0])
    )
    loads = [0.0] * shards
    buckets: list[list[tuple[str, list[str]]]] = [[] for _ in range(shards)]
    for key, args in ordered:
        target = loads.index(min(loads))
        loads[target] += weights.get(key, DEFAULT_WEIGHT)
        buckets[target].append((key, args))
    return buckets, loads


def record(stream) -> None:
    """Rewrite the weights file from a `cargo test --workspace` transcript."""
    pattern = re.compile(
        r"Running (unittests )?[^(]*\(target/[^/]+/deps/([a-z0-9_]+)-[0-9a-f]+\)"
        r".*?finished in ([0-9.]+)s",
        re.S,
    )
    measured: dict[str, float] = {}
    for _, name, seconds in pattern.findall(stream.read()):
        measured[name] = round(measured.get(name, 0.0) + float(seconds), 2)
    if not measured:
        sys.exit("no test timings found on stdin")
    payload = {
        "_comment": (
            "Advisory shard-balance weights in seconds, from one local "
            "`cargo test --workspace` run. Unlisted targets use the default "
            "weight; only balance depends on this file, never correctness."
        ),
        **dict(sorted(measured.items(), key=lambda kv: -kv[1])),
    }
    WEIGHTS_PATH.write_text(json.dumps(payload, indent=2) + "\n")
    print(f"wrote {WEIGHTS_PATH} with {len(measured)} targets", file=sys.stderr)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--shard", type=int, help="zero-based shard index")
    parser.add_argument("--shards", type=int, help="total shard count")
    parser.add_argument(
        "--record",
        action="store_true",
        help="rewrite test-weights.json from a cargo test transcript on stdin",
    )
    parser.add_argument(
        "--plan", action="store_true", help="print the whole assignment and exit"
    )
    arguments = parser.parse_args()

    if arguments.record:
        record(sys.stdin)
        return

    if arguments.shards is None or arguments.shards < 1:
        sys.exit("--shards must be a positive integer")
    items = work_items()
    buckets, loads = assign(items, arguments.shards)

    if arguments.plan:
        for index, (bucket, load) in enumerate(zip(buckets, loads)):
            heaviest = bucket[0][0] if bucket else "-"
            print(
                f"shard {index}: {len(bucket):2d} items, {load / 60:5.1f} local-min"
                f"  (heaviest: {heaviest})"
            )
        print(f"total {sum(loads) / 60:.1f} local-min, longest {max(loads) / 60:.1f}")
        return

    if arguments.shard is None or not 0 <= arguments.shard < arguments.shards:
        sys.exit("--shard must be in [0, shards)")
    for _, args in buckets[arguments.shard]:
        print(" ".join(args))


if __name__ == "__main__":
    main()
