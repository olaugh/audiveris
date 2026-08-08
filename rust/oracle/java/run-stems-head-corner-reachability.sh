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
probe_source="$repo_root/rust/oracle/java/StemsHeadCornerReachabilityProbe.java"
runner_source="$repo_root/rust/oracle/java/run-stems-head-corner-reachability.sh"

if [ ! -f "$probe_cp_file" ]; then
    echo "missing saved app runtime classpath: $probe_cp_file" >&2
    exit 2
fi

probe_classes=$(mktemp -d /private/tmp/stems-head-corner-reach-classes.XXXXXX)
probe_output=$(mktemp /private/tmp/stems-head-corner-reach-output.XXXXXX)
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
            -XX:+UnlockExperimentalVMOptions \
            -XX:+UseEpsilonGC \
            -Xmx4g \
            -Djava.awt.headless=true \
            -Dlogback.configurationFile="$repo_root/rust/oracle/java/logback-quiet.xml" \
            -cp "$probe_classes:$probe_cp" \
            org.audiveris.omr.rustport.StemsHeadCornerReachabilityProbe "$target"
    )
}

full_corpus=true
if [ "$#" -gt 0 ]; then
    full_corpus=false
fi

{
    env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
        -XX:+UnlockExperimentalVMOptions \
        -XX:+UseEpsilonGC \
        -Xmx4g \
        -cp "$probe_classes:$probe_cp" \
        org.audiveris.omr.rustport.StemsHeadCornerReachabilityProbe --header

    if [ "$full_corpus" = true ]; then
        run_probe "$repo_root/data/examples/chula.png:1"
        run_probe "$repo_root/data/examples/allegretto.png:1"
        run_probe "$repo_root/data/examples/batuque.png:1"
        run_probe "$repo_root/data/examples/carmen.png:1"
        run_probe "$repo_root/data/examples/cucaracha.png:1"
        run_probe "$repo_root/data/examples/hove.png:1"
        run_probe "$repo_root/data/examples/zizi.png:1"
        run_probe "$repo_root/data/examples/BachInvention5.jpg:1"
    else
        for target in "$@"; do
            run_probe "$target"
        done
    fi
} > "$probe_output"

summary_counts=$(awk '
    /^stemsheadreachpagesummary / {
        pages++
        for (i = 3; i <= NF; i += 2) {
            key = $i
            value = $(i + 1)
            if (key == "systems") systems += value
            else if (key == "heads") heads += value
            else if (key == "corners") corners += value
            else if (key == "seedsScanned") seedsScanned += value
            else if (key == "seedsKept") seedsKept += value
            else if (key == "headScans") headScans += value
            else if (key == "headTargets") headTargets += value
            else if (key == "siblingScans") siblingScans += value
            else if (key == "beamTargets") beamTargets += value
            else if (key == "anchorsCreated") anchorsCreated += value
            else if (key == "cSeedWrites") cSeedWrites += value
            else if (key == "builderChecks") builderChecks += value
        }
    }
    END {
        printf "%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d\n", pages, systems, heads,
            corners, seedsScanned, seedsKept, headScans, headTargets, siblingScans,
            beamTargets, anchorsCreated, cSeedWrites, builderChecks
    }
' "$probe_output")
IFS=: read -r pages systems heads corners seeds_scanned seeds_kept head_scans head_targets \
    sibling_scans beam_targets anchors_created c_seed_writes builder_checks <<EOF
$summary_counts
EOF

row_counts=$(awk '
    /^stemsheadreachpage / { pages++ }
    /^stemsheadreachsystem / { systems++ }
    /^stemsheadreachhead / { heads++ }
    /^stemsheadreachcorner / { corners++ }
    /^stemsheadreachseedscan / { seedScans++ }
    /^stemsheadreachseed / { seeds++ }
    /^stemsheadreachheadscan / { headScanRows++ }
    /^stemsheadreachheadtarget / { headTargets++ }
    /^stemsheadreachsiblings / { siblings++ }
    /^stemsheadreachbeamtarget / { beamTargets++ }
    /^stemsheadreachresult / { results++ }
    /^stemsheadreachbarena / { arenas++ }
    /^stemsheadreachsystemsummary / { systemSummaries++ }
    /^stemsheadreachpagesummary / { pageSummaries++ }
    END {
        printf "%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d\n", pages, systems,
            heads, corners, seedScans, seeds, headScanRows, headTargets, siblings,
            beamTargets, results, arenas, systemSummaries, pageSummaries
    }
' "$probe_output")
IFS=: read -r row_pages row_systems row_heads row_corners row_seed_scans row_seeds \
    row_head_scans row_head_targets row_siblings row_beam_targets row_results row_arenas \
    row_system_summaries row_page_summaries <<EOF
$row_counts
EOF

if [ "$row_pages:$row_systems:$row_heads:$row_corners:$row_seed_scans:$row_seeds:$row_head_scans:$row_head_targets:$row_beam_targets:$row_results:$row_arenas:$row_system_summaries:$row_page_summaries" != \
     "$pages:$systems:$heads:$corners:$corners:$seeds_kept:$corners:$head_targets:$beam_targets:$corners:$((2 * systems)):$systems:$pages" ]; then
    echo "STEMS head-reachability detail rows disagree with summaries" >&2
    exit 1
fi

if [ "$corners" -ne $((4 * heads)) ] \
        || [ "$c_seed_writes" -ne "$corners" ] \
        || [ "$row_siblings" -gt "$corners" ]; then
    echo "STEMS head-reachability coverage invariants failed" >&2
    exit 1
fi

bad_systems=$(awk '
    /^stemsheadreachsystemsummary / {
        for (i = 5; i <= NF; i += 2) fields[$i] = $(i + 1)
        if (fields["corners"] != 4 * fields["heads"] ||
                fields["cSeedWrites"] != fields["corners"] ||
                fields["builderChecks"] < fields["corners"]) bad++
        delete fields
    }
    END { print bad + 0 }
' "$probe_output")
if [ "$bad_systems" -ne 0 ]; then
    echo "$bad_systems STEMS head-reachability systems violate invariants" >&2
    exit 1
fi

probe_source_sha256=$(shasum -a 256 "$probe_source" | awk '{print $1}')
runner_source_sha256=$(shasum -a 256 "$runner_source" | awk '{print $1}')
body_sha256=$(shasum -a 256 "$probe_output" | awk '{print $1}')

if [ "$full_corpus" = true ]; then
    expected_totals="8:30:3521:14084:36736:1340:1007081:4566:9015:8120:1687:14084:16501"
    observed_totals="$pages:$systems:$heads:$corners:$seeds_scanned:$seeds_kept:$head_scans:$head_targets:$sibling_scans:$beam_targets:$anchors_created:$c_seed_writes:$builder_checks"
    expected_probe_source_sha256=116241d51a2f52668bdf31b4e7d5abda0191242d9dddeec2c04d17f31977d772
    expected_body_sha256=30a3b550f375b02491c2aa9a2fe4bc5b147454ac36ae4a6feb971aeb6d6d6399
    if [ "$observed_totals" != "$expected_totals" ]; then
        echo "unexpected STEMS head-reachability corpus totals" >&2
        echo "expected: $expected_totals" >&2
        echo "observed: $observed_totals" >&2
        exit 1
    fi
    if [ "$probe_source_sha256:$body_sha256" != \
         "$expected_probe_source_sha256:$expected_body_sha256" ]; then
        echo "unexpected STEMS head-reachability source/body hash" >&2
        echo "observed: $probe_source_sha256:$body_sha256" >&2
        exit 1
    fi
fi

cat "$probe_output"
printf 'stemsheadreachcorpussummary pages %d systems %d heads %d corners %d seedsScanned %d seedsKept %d headScans %d headTargets %d siblingScans %d beamTargets %d anchorsCreated %d cSeedWrites %d builderChecks %d probeSourceSha256 %s runnerSourceSha256 %s emittedBodySha256 %s\n' \
    "$pages" "$systems" "$heads" "$corners" "$seeds_scanned" "$seeds_kept" \
    "$head_scans" "$head_targets" "$sibling_scans" "$beam_targets" \
    "$anchors_created" "$c_seed_writes" "$builder_checks" "$probe_source_sha256" \
    "$runner_source_sha256" "$body_sha256"
