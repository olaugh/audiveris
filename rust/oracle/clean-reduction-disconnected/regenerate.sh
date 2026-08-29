#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
set -eu

fixture_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
svg_path="$fixture_dir/disconnected.svg"

verovio \
    --font Bravura \
    --page-width 2100 \
    --page-height 2970 \
    --scale 60 \
    --adjust-page-height \
    -o "$svg_path" \
    "$fixture_dir/piano-disconnected-barlines.mei"

rsvg-convert --background-color white --zoom 1 \
    -o "$fixture_dir/disconnected-1x.png" "$svg_path"
rsvg-convert --background-color white --zoom 1.5 \
    -o "$fixture_dir/disconnected-1_5x.png" "$svg_path"
rsvg-convert --background-color white --zoom 2 \
    -o "$fixture_dir/disconnected-2x.png" "$svg_path"

rm "$svg_path"
shasum -a 256 "$fixture_dir"/disconnected-*.png
