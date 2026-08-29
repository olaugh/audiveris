#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
set -eu

fixture_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
svg_path="$fixture_dir/coverage.svg"

verovio \
    --font Bravura \
    --page-width 2100 \
    --page-height 2970 \
    --scale 60 \
    --adjust-page-height \
    -o "$svg_path" \
    "$fixture_dir/piano-control-notation-coverage.mei"
rsvg-convert --background-color white --zoom 2 \
    -o "$fixture_dir/coverage-2x.png" "$svg_path"
rm "$svg_path"
shasum -a 256 "$fixture_dir/coverage-2x.png"
