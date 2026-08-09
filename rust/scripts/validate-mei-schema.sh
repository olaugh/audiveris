#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later

set -eu

expected_schema_sha256=f6440d5eb59c3e903f2a7a64ea26518646186f8449b86096106846a269eb354b
expected_license_sha256=f8303e68f99ec4056b4833dd3626ea661adf443bf0b8fbbebea0856898230db4

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
rust_root=$(dirname "$script_dir")
schema="$rust_root/schema/mei/mei-CMN-5.1.rng"
license="$rust_root/schema/mei/LICENSE-ECL-2.0.txt"
golden_root="$rust_root/crates/audiveris-mei/tests"

fail() {
    printf 'MEI schema validation failed: %s\n' "$*" >&2
    exit 1
}

sha256_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{print $1}'
    else
        fail "neither sha256sum nor shasum is available"
    fi
}

[ -f "$schema" ] || fail "schema is missing: $schema"
[ -f "$license" ] || fail "schema license is missing: $license"

actual_schema_sha256=$(sha256_file "$schema")
[ "$actual_schema_sha256" = "$expected_schema_sha256" ] ||
    fail "schema SHA-256 is $actual_schema_sha256, expected $expected_schema_sha256"
actual_license_sha256=$(sha256_file "$license")
[ "$actual_license_sha256" = "$expected_license_sha256" ] ||
    fail "license SHA-256 is $actual_license_sha256, expected $expected_license_sha256"

command -v xmllint >/dev/null 2>&1 ||
    fail "xmllint is required (install libxml2-utils on Linux)"
[ -d "$golden_root" ] || fail "golden directory is missing: $golden_root"

non_regular_goldens=$(find "$golden_root" -name '*.mei' ! -type f -print | LC_ALL=C sort)
[ -z "$non_regular_goldens" ] ||
    fail "non-regular .mei golden found: $non_regular_goldens"
goldens=$(find "$golden_root" -type f -name '*.mei' -print | LC_ALL=C sort)
[ -n "$goldens" ] || fail "no checked-in .mei goldens found below $golden_root"

validated=0
while IFS= read -r golden; do
    [ -n "$golden" ] || continue
    xmllint --nonet --noout --relaxng "$schema" "$golden" ||
        fail "Relax NG validation rejected $golden"
    validated=$((validated + 1))
done <<EOF
$goldens
EOF

printf 'Validated %s MEI golden(s) with MEI 5.1 CMN schema %s.\n' \
    "$validated" "$expected_schema_sha256"
