#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
set -eu

case ${1:-} in
    "") full_trace=false ;;
    --full-trace) full_trace=true ;;
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

probe_classes=$(mktemp -d /private/tmp/heads-seed-pass-classes.XXXXXX)
probe_output=$(mktemp /private/tmp/heads-seed-pass-output.XXXXXX)
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
            org.audiveris.omr.rustport.HeadsScannerContextProbe --seed-heads "$target"
    )
}

{
    env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
        -cp "$probe_classes:$probe_cp" \
        org.audiveris.omr.rustport.HeadsScannerContextProbe --seed-heads-header

    run_probe "$repo_root/data/examples/chula.png:1"
    run_probe "$repo_root/data/examples/allegretto.png:1"
    run_probe "$repo_root/data/examples/batuque.png:1"
    run_probe "$repo_root/data/examples/carmen.png:1"
    run_probe "$repo_root/data/examples/cucaracha.png:1"
    run_probe "$repo_root/data/examples/hove.png:1"
    run_probe "$repo_root/data/examples/zizi.png:1"
    run_probe "$repo_root/data/examples/BachInvention5.jpg:1"
} > "$probe_output"

pages=$(grep -c '^headseedpagesummary ' "$probe_output")
systems=$(grep -c '^headseedsystemsummary ' "$probe_output")
staffs=$(grep -c '^headseedstaffsummary ' "$probe_output")
attempts=$(grep -c '^headseedattempt ' "$probe_output")
candidates=$(grep -c '^headseedcandidate ' "$probe_output")
heads=$(grep -c '^headseedhead ' "$probe_output")
if [ "$pages:$systems:$staffs:$attempts:$candidates:$heads" != "8:30:55:61372:3435:3435" ]; then
    echo "unexpected seed-pass corpus totals $pages:$systems:$staffs:$attempts:$candidates:$heads" >&2
    exit 1
fi
body_sha256=$(shasum -a 256 "$probe_output" | awk '{print $1}')
if [ "$full_trace" = true ]; then
    cat "$probe_output"
else
    grep -v '^headseedattempt ' "$probe_output"
fi
printf 'headseedcorpussummary pages %d systems %d staffs %d attempts %d candidates %d heads %d fullBodySha256 %s\n' \
    "$pages" "$systems" "$staffs" "$attempts" "$candidates" "$heads" "$body_sha256"
