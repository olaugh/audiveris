#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
set -eu

if [ "$#" -ne 0 ]; then
    echo "usage: $0" >&2
    exit 2
fi

if [ -z "${JAVA_HOME:-}" ]; then
    echo "JAVA_HOME must name the frozen Temurin JDK 25" >&2
    exit 2
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
probe_cp_file=${AUDIVERIS_PROBE_CLASSPATH_FILE:-/private/tmp/audiveris-probe.classpath}
probe_source="$repo_root/rust/oracle/java/StemsHeadCornerProbe.java"

if [ ! -f "$probe_cp_file" ]; then
    echo "missing saved app runtime classpath: $probe_cp_file" >&2
    exit 2
fi

probe_classes=$(mktemp -d /private/tmp/stems-head-corners-classes.XXXXXX)
probe_output=$(mktemp /private/tmp/stems-head-corners-output.XXXXXX)
trap 'rm -rf "$probe_classes"; rm -f "$probe_output"' EXIT HUP INT TERM
probe_cp=$(sed -n '1p' "$probe_cp_file")

"$JAVA_HOME/bin/javac" \
    -cp "$probe_cp" \
    -d "$probe_classes" \
    "$probe_source"

run_probe ()
{
    target=$1
    (
        cd "$repo_root/app"
        env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
            -Djava.awt.headless=true \
            -Dlogback.configurationFile="$repo_root/rust/oracle/java/logback-quiet.xml" \
            -cp "$probe_classes:$probe_cp" \
            org.audiveris.omr.rustport.StemsHeadCornerProbe "$target"
    )
}

{
    env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
        -cp "$probe_classes:$probe_cp" \
        org.audiveris.omr.rustport.StemsHeadCornerProbe --header

    run_probe "$repo_root/data/examples/chula.png:1"
    run_probe "$repo_root/data/examples/allegretto.png:1"
    run_probe "$repo_root/data/examples/batuque.png:1"
    run_probe "$repo_root/data/examples/carmen.png:1"
    run_probe "$repo_root/data/examples/cucaracha.png:1"
    run_probe "$repo_root/data/examples/hove.png:1"
    run_probe "$repo_root/data/examples/zizi.png:1"
    run_probe "$repo_root/data/examples/BachInvention5.jpg:1"
} > "$probe_output"

pages=$(grep -c '^stemcornerpagesummary ' "$probe_output")
systems=$(grep -c '^stemcornersystemsummary ' "$probe_output")
heads=$(grep -c '^stemcornerhead ' "$probe_output")
corners=$(grep -c '^stemcorner ' "$probe_output")
totals=$(awk '
    /^stemcornerpagesummary / {
        for (i = 1; i <= NF; i++) {
            if ($i == "heads") heads += $(i + 1)
            else if ($i == "corners") corners += $(i + 1)
        }
    }
    END { printf "%d:%d", heads, corners }
' "$probe_output")

if [ "$pages:$systems:$heads:$corners:$totals" != "8:30:3521:14084:3521:14084" ]; then
    echo "unexpected STEMS head-corner corpus totals $pages:$systems:$heads:$corners:$totals" >&2
    exit 1
fi

bad_systems=$(awk '
    /^stemcornersystemsummary / {
        heads = corners = -1
        for (i = 1; i <= NF; i++) {
            if ($i == "heads") heads = $(i + 1)
            else if ($i == "corners") corners = $(i + 1)
        }
        if (corners != 4 * heads) bad++
    }
    END { print bad + 0 }
' "$probe_output")
if [ "$bad_systems" -ne 0 ]; then
    echo "$bad_systems system summaries do not have four corners per head" >&2
    exit 1
fi

probe_source_sha256=$(shasum -a 256 "$probe_source" | awk '{print $1}')
body_sha256=$(shasum -a 256 "$probe_output" | awk '{print $1}')
cat "$probe_output"
printf 'stemcornercorpussummary pages %d systems %d heads %d corners %d probeSourceSha256 %s emittedBodySha256 %s\n' \
    "$pages" "$systems" "$heads" "$corners" "$probe_source_sha256" "$body_sha256"
