#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
output="$repo_root/rust/crates/audiveris-omr/src/data/bravura-head-templates-practical.bin"
point_sizes=24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,55,56,57,58,59,60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75,76,77,79,80,81,82,86,88,89,90,91,92,93,94,95,96,97,98,99,100,101,102,103,104,105,106,107,108,109,110,111,112,113,114,115,116,117,118,119,120,121,122,123,124,125,126,127,128
oracle=$(mktemp /private/tmp/bravura-practical-head-templates.XXXXXX)
trap 'rm -f "$oracle"' EXIT HUP INT TERM

"$script_dir/run-head-template-catalog.sh" --point-sizes "$point_sizes" > "$oracle"

check=
if [ "${1:-}" = "--check" ]; then
    check=--check
elif [ "$#" -ne 0 ]; then
    echo "expected no arguments or --check" >&2
    exit 2
fi

python3 "$repo_root/rust/oracle/generate-head-template-data.py" \
    "$oracle" \
    "$output" \
    --expected-pages 97 \
    --expected-point-sizes "$point_sizes" \
    --expected-page-pixels any \
    $check
