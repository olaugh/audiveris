#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
set -eu

case ${1:-} in
    "") probe_args= ;;
    --full-trace) probe_args=--full-trace ;;
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

probe_classes=$(mktemp -d /private/tmp/heads-post-range-classes.XXXXXX)
probe_output=$(mktemp /private/tmp/heads-post-range-output.XXXXXX)
trap 'rm -rf "$probe_classes"; rm -f "$probe_output"' EXIT HUP INT TERM
probe_cp=$(sed -n '1p' "$probe_cp_file")

"$JAVA_HOME/bin/javac" \
    -cp "$probe_cp" \
    -d "$probe_classes" \
    "$repo_root/rust/oracle/java/HeadsPostRangeProbe.java"

run_probe ()
{
    target=$1
    (
        cd "$repo_root/app"
        if [ -n "$probe_args" ]; then
            env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
                -Djava.awt.headless=true \
                -Dlogback.configurationFile="$repo_root/rust/oracle/java/logback-quiet.xml" \
                -cp "$probe_classes:$probe_cp" \
                org.audiveris.omr.rustport.HeadsPostRangeProbe "$probe_args" "$target"
        else
            env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
                -Djava.awt.headless=true \
                -Dlogback.configurationFile="$repo_root/rust/oracle/java/logback-quiet.xml" \
                -cp "$probe_classes:$probe_cp" \
                org.audiveris.omr.rustport.HeadsPostRangeProbe "$target"
        fi
    )
}

{
    env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
        -cp "$probe_classes:$probe_cp" \
        org.audiveris.omr.rustport.HeadsPostRangeProbe --header

    run_probe "$repo_root/data/examples/chula.png:1"
    run_probe "$repo_root/data/examples/allegretto.png:1"
    run_probe "$repo_root/data/examples/batuque.png:1"
    run_probe "$repo_root/data/examples/carmen.png:1"
    run_probe "$repo_root/data/examples/cucaracha.png:1"
    run_probe "$repo_root/data/examples/hove.png:1"
    run_probe "$repo_root/data/examples/zizi.png:1"
    run_probe "$repo_root/data/examples/BachInvention5.jpg:1"
} > "$probe_output"

pages=$(grep -c '^headpostpagesummary ' "$probe_output")
systems=$(grep -c '^headpostsystemsummary ' "$probe_output")
staffs=$(grep -c '^headpoststaffsummary ' "$probe_output")
duplicates=$(grep -c '^headpostduplicate ' "$probe_output")
overlaps=$(grep -c '^headpostoverlap ' "$probe_output")
staff_heads=$(grep -c '^headpoststaffhead ' "$probe_output")
beam_inputs=$(grep -c '^headpostbeaminput ' "$probe_output" || true)
beam_checks=$(grep -c '^headpostbeamcheck ' "$probe_output" || true)
beam_decisions=$(grep -c '^headpostbeamdecision ' "$probe_output" || true)
beam_purges=$(grep -c '^headpostbeampurge ' "$probe_output" || true)
head_purges=$(grep -c '^headpostheadpurge ' "$probe_output" || true)
final_heads=$(grep -c '^headpostsystemhead ' "$probe_output")
scale_inputs=$(grep -c '^headpostscaleinput ' "$probe_output" || true)
scale_rows=$(grep -c '^headpostscale ' "$probe_output" || true)
totals=$(awk '
    /^headpostpagesummary / {
        for (i = 1; i <= NF; i++) {
            if ($i == "inputs") inputs += $(i + 1)
            else if ($i == "duplicates") duplicates += $(i + 1)
            else if ($i == "overlaps") overlaps += $(i + 1)
            else if ($i == "staffHeads") staffHeads += $(i + 1)
            else if ($i == "purgedBeams") beams += $(i + 1)
            else if ($i == "purgedHeads") heads += $(i + 1)
            else if ($i == "finalHeads") finalHeads += $(i + 1)
        }
    }
    END { printf "%d:%d:%d:%d:%d:%d:%d", inputs, duplicates, overlaps, staffHeads, beams, heads, finalHeads }
' "$probe_output")

if [ "$pages:$systems:$staffs:$totals:$duplicates:$overlaps:$staff_heads:$beam_inputs:$beam_decisions:$beam_purges:$head_purges:$final_heads:$scale_inputs:$scale_rows" \
        != "8:30:55:3609:62:2725:3547:0:26:3521:62:2725:3547:191:26:0:26:3521:1451:18" ]; then
    echo "unexpected post-range corpus totals " \
        "$pages:$systems:$staffs:$totals:$duplicates:$overlaps:$staff_heads:$beam_inputs:$beam_decisions:$beam_purges:$head_purges:$final_heads:$scale_inputs:$scale_rows" >&2
    exit 1
fi

body_sha256=$(shasum -a 256 "$probe_output" | awk '{print $1}')
cat "$probe_output"
printf 'headpostcorpussummary pages %d systems %d staffs %d totals:inputs:duplicates:overlaps:staffHeads:purgedBeams:purgedHeads:finalHeads %s decisionRows %d:%d staffHeadRows %d beamInputRows %d beamChecks %d beamDecisionRows %d beamPurgeRows %d headPurgeRows %d finalHeadRows %d scaleInputRows %d scaleRows %d emittedBodySha256 %s\n' \
    "$pages" "$systems" "$staffs" "$totals" "$duplicates" "$overlaps" "$staff_heads" "$beam_inputs" "$beam_checks" "$beam_decisions" "$beam_purges" "$head_purges" "$final_heads" "$scale_inputs" "$scale_rows" "$body_sha256"
