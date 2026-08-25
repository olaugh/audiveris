#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
set -eu

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

probe_classes=$(mktemp -d /private/tmp/head-template-catalog-classes.XXXXXX)
trap 'rm -rf "$probe_classes"' EXIT HUP INT TERM
probe_cp=$(sed -n '1p' "$probe_cp_file")

"$JAVA_HOME/bin/javac" \
    -cp "$probe_cp" \
    -d "$probe_classes" \
    "$repo_root/rust/oracle/java/HeadTemplateCatalogProbe.java"

run_probe ()
{
    target=$1
    small_heads=${AUDIVERIS_SMALL_HEADS:-false}
    (
        cd "$repo_root/app"
        if [ "$small_heads" = true ]; then
            env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
                -Djava.awt.headless=true \
                -Dlogback.configurationFile="$repo_root/rust/oracle/java/logback-quiet.xml" \
                -cp "$probe_classes:$probe_cp" \
                org.audiveris.omr.rustport.HeadTemplateCatalogProbe --small-heads "$target"
        else
            env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
                -Djava.awt.headless=true \
                -Dlogback.configurationFile="$repo_root/rust/oracle/java/logback-quiet.xml" \
                -cp "$probe_classes:$probe_cp" \
                org.audiveris.omr.rustport.HeadTemplateCatalogProbe "$target"
        fi
    )
}

env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
    -cp "$probe_classes:$probe_cp" \
    org.audiveris.omr.rustport.HeadTemplateCatalogProbe --header

if [ "$#" -gt 0 ]; then
    for target in "$@"; do
        run_probe "$target"
    done
else
    run_probe "$repo_root/data/examples/chula.png:1"
    run_probe "$repo_root/data/examples/allegretto.png:1"
    run_probe "$repo_root/data/examples/batuque.png:1"
    run_probe "$repo_root/data/examples/carmen.png:1"
    run_probe "$repo_root/data/examples/cucaracha.png:1"
    run_probe "$repo_root/data/examples/hove.png:1"
    run_probe "$repo_root/data/examples/zizi.png:1"
    run_probe "$repo_root/data/examples/BachInvention5.jpg:1"
fi
