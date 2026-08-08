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
probe_source="$repo_root/rust/oracle/java/StemsHeadSeedProbe.java"

if [ ! -f "$probe_cp_file" ]; then
    echo "missing saved app runtime classpath: $probe_cp_file" >&2
    exit 2
fi

probe_classes=$(mktemp -d /private/tmp/stems-head-seeds-classes.XXXXXX)
probe_output=$(mktemp /private/tmp/stems-head-seeds-output.XXXXXX)
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
            org.audiveris.omr.rustport.StemsHeadSeedProbe "$target"
    )
}

{
    env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
        -cp "$probe_classes:$probe_cp" \
        org.audiveris.omr.rustport.StemsHeadSeedProbe --header

    run_probe "$repo_root/data/examples/chula.png:1"
    run_probe "$repo_root/data/examples/allegretto.png:1"
    run_probe "$repo_root/data/examples/batuque.png:1"
    run_probe "$repo_root/data/examples/carmen.png:1"
    run_probe "$repo_root/data/examples/cucaracha.png:1"
    run_probe "$repo_root/data/examples/hove.png:1"
    run_probe "$repo_root/data/examples/zizi.png:1"
    run_probe "$repo_root/data/examples/BachInvention5.jpg:1"
} > "$probe_output"

row_counts=$(awk '
    /^stemsheadseedpagesummary / { pages++ }
    /^stemsheadseedsystemsummary / { systems++ }
    /^stemsheadseednostem / { noStemAreas++ }
    /^stemsheadseedfree / { sourceSeeds++ }
    /^stemsheadseedpurgecheck / { purgeChecks++ }
    /^stemsheadseedhead / { heads++ }
    /^stemsheadseedcorner / { corners++ }
    /^stemsheadseedneighbor / { neighborRows++ }
    /^stemsheadseedcandidate / { candidates++ }
    /^stemsheadseeddecision / {
        decisions++
        for (i = 1; i <= NF; i++) {
            if ($i == "visited") visited += $(i + 1)
            else if ($i == "outcome") {
                if ($(i + 1) == "fallback") fallback++
                else if ($(i + 1) ~ /^selected:/) selected++
            }
        }
    }
    END {
        printf "%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d", pages, systems,
            sourceSeeds, noStemAreas, purgeChecks, heads, corners, neighborRows,
            candidates, visited, decisions, selected, fallback
    }
' "$probe_output")

summary_counts=$(awk '
    /^stemsheadseedpagesummary / {
        for (i = 1; i <= NF; i++) {
            if ($i == "sourceSeeds") sourceSeeds += $(i + 1)
            else if ($i == "keptSeeds") keptSeeds += $(i + 1)
            else if ($i == "purgedSeeds") purgedSeeds += $(i + 1)
            else if ($i == "noStemAreas") noStemAreas += $(i + 1)
            else if ($i == "purgeChecks") purgeChecks += $(i + 1)
            else if ($i == "heads") heads += $(i + 1)
            else if ($i == "corners") corners += $(i + 1)
            else if ($i == "neighborRows") neighborRows += $(i + 1)
            else if ($i == "candidates") candidates += $(i + 1)
            else if ($i == "visited") visited += $(i + 1)
            else if ($i == "selected") selected += $(i + 1)
            else if ($i == "fallback") fallback += $(i + 1)
        }
    }
    END {
        printf "%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d", sourceSeeds, keptSeeds,
            purgedSeeds, noStemAreas, purgeChecks, heads, corners, neighborRows,
            candidates, visited, selected, fallback
    }
' "$probe_output")

IFS=: read -r pages systems row_source row_areas row_purge_checks row_heads row_corners \
    row_neighbors row_candidates row_visited decisions row_selected row_fallback <<EOF
$row_counts
EOF
IFS=: read -r source_seeds kept_seeds purged_seeds no_stem_areas purge_checks heads corners \
    neighbor_rows candidates visited selected fallback <<EOF
$summary_counts
EOF

if [ "$pages:$systems:$heads:$corners:$decisions" != "8:30:3521:14084:14084" ]; then
    echo "unexpected STEMS head-seed corpus shape $pages:$systems:$heads:$corners:$decisions" >&2
    exit 1
fi

if [ "$row_source:$row_areas:$row_purge_checks:$row_heads:$row_corners:$row_neighbors:$row_candidates:$row_visited:$row_selected:$row_fallback" != \
     "$source_seeds:$no_stem_areas:$purge_checks:$heads:$corners:$neighbor_rows:$candidates:$visited:$selected:$fallback" ]; then
    echo "STEMS head-seed row counts disagree with page summaries" >&2
    exit 1
fi

if [ $((kept_seeds + purged_seeds)) -ne "$source_seeds" ]; then
    echo "kept and purged seed totals do not cover the source pool" >&2
    exit 1
fi

if [ $((selected + fallback)) -ne "$corners" ]; then
    echo "selected and fallback totals do not cover all corners" >&2
    exit 1
fi

if [ "$visited" -gt "$candidates" ]; then
    echo "visited seed candidates exceed sorted candidates" >&2
    exit 1
fi

bad_systems=$(awk '
    /^stemsheadseedsystemsummary / {
        heads = corners = selected = fallback = -1
        for (i = 1; i <= NF; i++) {
            if ($i == "heads") heads = $(i + 1)
            else if ($i == "corners") corners = $(i + 1)
            else if ($i == "selected") selected = $(i + 1)
            else if ($i == "fallback") fallback = $(i + 1)
        }
        if ((corners != 4 * heads) || (corners != selected + fallback)) bad++
    }
    END { print bad + 0 }
' "$probe_output")
if [ "$bad_systems" -ne 0 ]; then
    echo "$bad_systems system summaries violate corner invariants" >&2
    exit 1
fi

probe_source_sha256=$(shasum -a 256 "$probe_source" | awk '{print $1}')
body_sha256=$(shasum -a 256 "$probe_output" | awk '{print $1}')
cat "$probe_output"
printf 'stemsheadseedcorpussummary pages %d systems %d sourceSeeds %d keptSeeds %d purgedSeeds %d noStemAreas %d purgeChecks %d heads %d corners %d neighborRows %d candidates %d visited %d selected %d fallback %d probeSourceSha256 %s emittedBodySha256 %s\n' \
    "$pages" "$systems" "$source_seeds" "$kept_seeds" "$purged_seeds" \
    "$no_stem_areas" "$purge_checks" "$heads" "$corners" "$neighbor_rows" \
    "$candidates" "$visited" "$selected" "$fallback" "$probe_source_sha256" "$body_sha256"
