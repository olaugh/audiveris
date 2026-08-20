#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic generic finalizeStems cleaner/canonical/abnormal evidence.
set -eu

if [ -z "${JAVA_HOME:-}" ] || [ ! -x "$JAVA_HOME/bin/java" ]; then
    echo "JAVA_HOME must name the frozen Temurin JDK 25" >&2
    exit 2
fi
release_field()
{
    awk -F= -v name="$1" '$1 == name { value = $2; gsub(/^"|"$/, "", value); print value }' \
        "$JAVA_HOME/release"
}
if [ "$(release_field IMPLEMENTOR)" != "Eclipse Adoptium" ] || \
        [ "$(release_field IMPLEMENTOR_VERSION)" != "Temurin-25.0.3+9" ] || \
        [ "$(release_field JAVA_RUNTIME_VERSION)" != "25.0.3+9-LTS" ] || \
        [ "$(release_field OS_NAME)" != "Darwin" ] || \
        [ "$(release_field OS_ARCH)" != "aarch64" ] || \
        [ "$(release_field JVM_VARIANT)" != "Hotspot" ]; then
    echo "JAVA_HOME is not frozen Temurin 25.0.3+9-LTS aarch64 HotSpot" >&2
    exit 2
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
audiveris_root=${AUDIVERIS_ROOT:-/Users/john/sources/aug04-rubigo/audiveris}
if [ "$(git -C "$audiveris_root" rev-parse HEAD)" != \
        "07b5f9f33822960c3b70bbea85e26f24ada08963" ]; then
    echo "Audiveris Java source is not the frozen generic-finalizer baseline" >&2
    exit 2
fi

tmp_dir=$(mktemp -d /private/tmp/stems-finalize-generic.XXXXXX)
trap 'rm -rf "$tmp_dir"' EXIT
warmup="$tmp_dir/warmup"
pass_one="$tmp_dir/pass1"
pass_two="$tmp_dir/pass2"
rows="$tmp_dir/rows"

run_pass()
{
    output=$1
    "$audiveris_root/gradlew" --no-daemon \
        -I "$script_dir/stems-finalize-generic.init.gradle" \
        -PrustPortRepo="$repo_root" \
        -PfinalizeAllegretto="$audiveris_root/data/examples/allegretto.png" \
        -PfinalizeZizi="$audiveris_root/data/examples/zizi.png" \
        -PfinalizeChula="$audiveris_root/data/examples/chula.png" \
        :app:finalizeStemsGenericProbe > "$output"
    grep '^finalizegeneric ' "$output"
}

run_pass "$warmup" >/dev/null
run_pass "$pass_one" > "$tmp_dir/semantic1"
run_pass "$pass_two" > "$tmp_dir/semantic2"
if ! cmp -s "$tmp_dir/semantic1" "$tmp_dir/semantic2"; then
    echo "generic finalizeStems fresh passes differ" >&2
    diff "$tmp_dir/semantic1" "$tmp_dir/semantic2" | head -20 >&2
    exit 1
fi
cp "$tmp_dir/semantic1" "$rows"

if [ "$(grep -c '^finalizegeneric before ' "$rows")" -ne 7 ] || \
        [ "$(grep -c '^finalizegeneric after ' "$rows")" -ne 4 ] || \
        [ "$(grep -c '^finalizegeneric synthetic ' "$rows")" -ne 1 ] || \
        ! grep -q '^finalizegeneric before page allegretto.png#1 system 1 head 1317 .* canonical false$' "$rows" || \
        ! grep -q '^finalizegeneric after page allegretto.png#1 system 1 removed \[head1317:stem2240:.*\] abnormal \[\] syntheticHead -$' "$rows" || \
        ! grep -q '^finalizegeneric before page zizi.png#1 system 2 head 1183 .* canonical false$' "$rows" || \
        ! grep -q '^finalizegeneric after page zizi.png#1 system 2 removed \[head1183:stem[0-9][0-9]*:sideLEFT:.*\] abnormal \[\] syntheticHead -$' "$rows" || \
        ! grep -q '^finalizegeneric synthetic page chula.png#1 system 1 .* abnormal true->false relations 0 ' "$rows" || \
        ! grep -q '^finalizegeneric after page chula.png#1 system 1 removed \[\] abnormal \[HeadInter[0-9][0-9]*:false->true\] syntheticHead [0-9][0-9]*$' "$rows"; then
    echo "generic finalizeStems fixture contract differs" >&2
    exit 1
fi

probe_sha=$(shasum -a 256 "$script_dir/FinalizeStemsGenericProbe.java" | awk '{print $1}')
init_sha=$(shasum -a 256 "$script_dir/stems-finalize-generic.init.gradle" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$rows" | awk '{print $1}')
row_count=$(wc -l < "$rows" | tr -d ' ')
out=/private/tmp/stems-finalize-generic-audit.txt
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) generic finalizeStems evidence.'
    echo '# schema: stems-finalize-generic-v1'
    echo '# Real cleaner removals and canonical keeps plus a controlled checkNeededStems abnormal reset.'
    cat "$rows"
    printf '%s\n' \
        "stemsfinalizegeneric summary schema stems-finalize-generic-v1 rows $row_count javaSourceCommit 07b5f9f33822960c3b70bbea85e26f24ada08963 probeSourceSha256 $probe_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha bodySha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope GenericHeadStemsCleanerAndNeededStems javaEvidence ReturnedAfterFinalizeStems"
} > "$out"
echo "wrote $out"
