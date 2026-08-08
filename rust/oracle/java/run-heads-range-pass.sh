#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
set -eu

case ${1:-} in
    "") probe_mode=--range-heads ;;
    --full-trace) probe_mode=--range-heads-full ;;
    *) echo "usage: $0 [--full-trace]" >&2; exit 2 ;;
esac

if [ -z "${JAVA_HOME:-}" ]; then
    echo "JAVA_HOME must name the frozen Temurin JDK 25" >&2
    exit 2
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
probe_cp_file=${AUDIVERIS_PROBE_CLASSPATH_FILE:-/private/tmp/audiveris-probe.classpath}

if [ ! -f "$probe_cp_file" ]; then
    echo "missing saved app runtime classpath: $probe_cp_file" >&2
    exit 2
fi

probe_classes=$(mktemp -d /private/tmp/heads-range-pass-classes.XXXXXX)
probe_output=$(mktemp /private/tmp/heads-range-pass-output.XXXXXX)
trap 'rm -rf "$probe_classes"; rm -f "$probe_output"' EXIT HUP INT TERM
probe_cp=$(sed -n '1p' "$probe_cp_file")

"$JAVA_HOME/bin/javac" \
    -cp "$probe_cp" \
    -d "$probe_classes" \
    "$repo_root/rust/oracle/java/HeadsScannerContextProbe.java"

run_probe ()
{
    target=$1
    (
        cd "$repo_root/app"
        env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
            -Djava.awt.headless=true \
            -Dlogback.configurationFile="$repo_root/rust/oracle/java/logback-quiet.xml" \
            -cp "$probe_classes:$probe_cp" \
            org.audiveris.omr.rustport.HeadsScannerContextProbe "$probe_mode" "$target"
    )
}

{
    env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
        -cp "$probe_classes:$probe_cp" \
        org.audiveris.omr.rustport.HeadsScannerContextProbe --range-heads-header

    run_probe "$repo_root/data/examples/chula.png:1"
    run_probe "$repo_root/data/examples/allegretto.png:1"
    run_probe "$repo_root/data/examples/batuque.png:1"
    run_probe "$repo_root/data/examples/carmen.png:1"
    run_probe "$repo_root/data/examples/cucaracha.png:1"
    run_probe "$repo_root/data/examples/hove.png:1"
    run_probe "$repo_root/data/examples/zizi.png:1"
    run_probe "$repo_root/data/examples/BachInvention5.jpg:1"
} > "$probe_output"

pages=$(grep -c '^headrangepagesummary ' "$probe_output")
systems=$(grep -c '^headrangesystemsummary ' "$probe_output")
staffs=$(grep -c '^headrangestaffsummary ' "$probe_output")
spot_slices=$(grep -c '^headrangespot ' "$probe_output")
seed_heads=$(grep -c '^headrangeseedhead ' "$probe_output")
candidates=$(grep -c '^headrangecandidate ' "$probe_output")
range_heads=$(grep -c '^headrangehead ' "$probe_output")
totals=$(awk '
    /^headrangestaffsummary / {
        for (i = 1; i <= NF; i++) {
            if ($i == "scans") scans += $(i + 1)
            else if ($i == "safetySkips") skips += $(i + 1)
            else if ($i == "attempts") attempts += $(i + 1)
            else if ($i == "rawCandidates") raw += $(i + 1)
            else if ($i == "seedConflicts") conflicts += $(i + 1)
            else if ($i == "glyphEmpty") empty += $(i + 1)
        }
    }
    END { printf "%d:%d:%d:%d:%d:%d", scans, skips, attempts, raw, conflicts, empty }
' "$probe_output")

if [ "$pages:$systems:$staffs:$spot_slices:$totals:$seed_heads:$candidates:$range_heads" \
        != "8:30:55:6759:921558:5389:3119882:34101:3376:0:3435:3550:174" ]; then
    echo "unexpected range-pass corpus totals " \
        "$pages:$systems:$staffs:$spot_slices:$totals:$seed_heads:$candidates:$range_heads" >&2
    exit 1
fi

body_sha256=$(shasum -a 256 "$probe_output" | awk '{print $1}')
cat "$probe_output"
printf 'headrangecorpussummary pages %d systems %d staffs %d spotSlices %d scans:skips:attempts:rawCandidates:seedConflicts:glyphEmpty %s seedHeads %d candidates %d rangeHeads %d emittedBodySha256 %s\n' \
    "$pages" "$systems" "$staffs" "$spot_slices" "$totals" "$seed_heads" "$candidates" "$range_heads" "$body_sha256"
