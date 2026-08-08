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
probe_source="$repo_root/rust/oracle/java/StemsBeamStumpProbe.java"
runner_source="$repo_root/rust/oracle/java/run-stems-beam-stumps.sh"

if [ ! -f "$probe_cp_file" ]; then
    echo "missing saved app runtime classpath: $probe_cp_file" >&2
    exit 2
fi

probe_classes=$(mktemp -d /private/tmp/stems-beam-stumps-classes.XXXXXX)
probe_output=$(mktemp /private/tmp/stems-beam-stumps-output.XXXXXX)
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
            org.audiveris.omr.rustport.StemsBeamStumpProbe "$target"
    )
}

{
    env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
        -XX:+UnlockExperimentalVMOptions \
        -XX:+UseEpsilonGC \
        -Xmx4g \
        -cp "$probe_classes:$probe_cp" \
        org.audiveris.omr.rustport.StemsBeamStumpProbe --header

    run_probe "$repo_root/data/examples/chula.png:1"
    run_probe "$repo_root/data/examples/allegretto.png:1"
    run_probe "$repo_root/data/examples/batuque.png:1"
    run_probe "$repo_root/data/examples/carmen.png:1"
    run_probe "$repo_root/data/examples/cucaracha.png:1"
    run_probe "$repo_root/data/examples/hove.png:1"
    run_probe "$repo_root/data/examples/zizi.png:1"
    run_probe "$repo_root/data/examples/BachInvention5.jpg:1"
} > "$probe_output"

summary_counts=$(awk '
    /^stemsbeamstumppagesummary / {
        pages++
        for (i = 1; i <= NF; i++) {
            if ($i == "systems") systems += $(i + 1)
            else if ($i == "constructors") constructors += $(i + 1)
            else if ($i == "sides") sides += $(i + 1)
            else if ($i == "neighbors") neighbors += $(i + 1)
            else if ($i == "seedInputs") seedInputs += $(i + 1)
            else if ($i == "purgeComparisons") purgeComparisons += $(i + 1)
            else if ($i == "purgeRemovals") purgeRemovals += $(i + 1)
            else if ($i == "purgeBreaks") purgeBreaks += $(i + 1)
            else if ($i == "sideSeeds") sideSeeds += $(i + 1)
            else if ($i == "buildAttempts") buildAttempts += $(i + 1)
            else if ($i == "emptySections") emptySections += $(i + 1)
            else if ($i == "zeroCompounds") zeroCompounds += $(i + 1)
            else if ($i == "candidates") candidates += $(i + 1)
            else if ($i == "directionAccepted") directionAccepted += $(i + 1)
            else if ($i == "directionRejected") directionRejected += $(i + 1)
            else if ($i == "registrations") registrations += $(i + 1)
            else if ($i == "newBuilds") newBuilds += $(i + 1)
            else if ($i == "reusedBuilds") reusedBuilds += $(i + 1)
            else if ($i == "sectionRows") sectionRows += $(i + 1)
            else if ($i == "steps") steps += $(i + 1)
            else if ($i == "finalStumps") finalStumps += $(i + 1)
            else if ($i == "finalSideStumps") finalSideStumps += $(i + 1)
            else if ($i == "tremolos") tremolos += $(i + 1)
        }
    }
    END {
        printf "%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d", \
            pages, systems, constructors, sides, neighbors, seedInputs, purgeComparisons, \
            purgeRemovals, purgeBreaks, sideSeeds, buildAttempts, emptySections, zeroCompounds, \
            candidates, directionAccepted, directionRejected, registrations, newBuilds, \
            reusedBuilds, sectionRows, steps, finalStumps, finalSideStumps, tremolos
    }
' "$probe_output")

IFS=: read -r pages systems constructors sides neighbors seed_inputs purge_comparisons \
    purge_removals purge_breaks side_seeds build_attempts empty_sections zero_compounds \
    candidates direction_accepted direction_rejected registrations new_builds reused_builds \
    section_rows steps final_stumps final_side_stumps tremolos <<EOF
$summary_counts
EOF

row_counts=$(awk '
    /^stemsbeamstumpsystemsummary / { systems++ }
    /^stemsbeamstumpbeam / { constructors++ }
    /^stemsbeamstumpneighbor / { neighbors++ }
    /^stemsbeamstumpseed / { seedInputs++ }
    /^stemsbeamstumppurge / { purgeComparisons++ }
    /^stemsbeamstumpside / { sides++ }
    /^stemsbeamstumpbuild / { buildAttempts++ }
    /^stemsbeamstumpsection / { sectionRows++ }
    /^stemsbeamstumpstep / { steps++ }
    /^stemsbeamstumpdirections / { candidates++ }
    /^stemsbeamstumpreg / { registrations++ }
    /^stemsbeamstumpfinal / { finalStumps++ }
    /^stemsbeamstumpfinalside / { finalSideRows++ }
    END {
        printf "%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d", systems, constructors, \
            neighbors, seedInputs, purgeComparisons, sides, buildAttempts, sectionRows, steps, \
            candidates, registrations, finalStumps, finalSideRows
    }
' "$probe_output")
IFS=: read -r row_systems row_constructors row_neighbors row_seed_inputs \
    row_purge_comparisons row_sides row_build_attempts row_section_rows row_steps \
    row_candidates row_registrations row_final_stumps row_final_side_rows <<EOF
$row_counts
EOF

if [ "$pages:$systems:$constructors:$sides" != "8:30:803:1606" ]; then
    echo "unexpected STEMS beam-stump corpus shape $pages:$systems:$constructors:$sides" >&2
    exit 1
fi
if [ "$row_systems:$row_constructors:$row_neighbors:$row_seed_inputs:$row_purge_comparisons:$row_sides:$row_build_attempts:$row_section_rows:$row_steps:$row_candidates:$row_registrations:$row_final_stumps:$row_final_side_rows" != \
     "$systems:$constructors:$neighbors:$seed_inputs:$purge_comparisons:$side_seeds:$build_attempts:$section_rows:$steps:$candidates:$registrations:$final_stumps:$sides" ]; then
    echo "STEMS beam-stump row counts disagree with page summaries" >&2
    exit 1
fi
if [ $((side_seeds + build_attempts)) -ne "$sides" ] \
        || [ $((empty_sections + zero_compounds + candidates)) -ne "$build_attempts" ] \
        || [ $((direction_accepted + direction_rejected)) -ne "$candidates" ] \
        || [ $((new_builds + reused_builds)) -ne "$direction_accepted" ] \
        || [ "$new_builds" -ne "$registrations" ] \
        || [ "$final_side_stumps" -ne $((side_seeds + direction_accepted)) ]; then
    echo "STEMS beam-stump outcome totals violate coverage invariants" >&2
    exit 1
fi
if [ "$constructors:$sides:$neighbors:$seed_inputs:$purge_comparisons:$purge_removals:$purge_breaks:$side_seeds:$build_attempts:$empty_sections:$zero_compounds:$candidates:$direction_accepted:$direction_rejected:$registrations:$new_builds:$reused_builds:$section_rows:$steps:$final_stumps:$final_side_stumps:$tremolos" != \
     "803:1606:3934:1820:1087:5:1082:1305:301:4:154:143:6:137:5:5:1:447:447:1821:1311:0" ]; then
    echo "unexpected STEMS beam-stump corpus totals" >&2
    exit 1
fi

bad_systems=$(awk '
    /^stemsbeamstumpsystemsummary / {
        constructors = sides = sideSeeds = buildAttempts = -1
        emptySections = zeroCompounds = candidates = -1
        accepted = rejected = newBuilds = reusedBuilds = registrations = -1
        finalSideStumps = -1
        for (i = 1; i <= NF; i++) {
            if ($i == "constructors") constructors = $(i + 1)
            else if ($i == "sides") sides = $(i + 1)
            else if ($i == "sideSeeds") sideSeeds = $(i + 1)
            else if ($i == "buildAttempts") buildAttempts = $(i + 1)
            else if ($i == "emptySections") emptySections = $(i + 1)
            else if ($i == "zeroCompounds") zeroCompounds = $(i + 1)
            else if ($i == "candidates") candidates = $(i + 1)
            else if ($i == "directionAccepted") accepted = $(i + 1)
            else if ($i == "directionRejected") rejected = $(i + 1)
            else if ($i == "registrations") registrations = $(i + 1)
            else if ($i == "newBuilds") newBuilds = $(i + 1)
            else if ($i == "reusedBuilds") reusedBuilds = $(i + 1)
            else if ($i == "finalSideStumps") finalSideStumps = $(i + 1)
        }
        if ((sides != 2 * constructors) || (sides != sideSeeds + buildAttempts) ||
                (buildAttempts != emptySections + zeroCompounds + candidates) ||
                (candidates != accepted + rejected) ||
                (accepted != newBuilds + reusedBuilds) || (registrations != newBuilds) ||
                (finalSideStumps != sideSeeds + accepted)) bad++
    }
    END { print bad + 0 }
' "$probe_output")
if [ "$bad_systems" -ne 0 ]; then
    echo "$bad_systems system summaries violate beam-stump invariants" >&2
    exit 1
fi

probe_source_sha256=$(shasum -a 256 "$probe_source" | awk '{print $1}')
runner_source_sha256=$(shasum -a 256 "$runner_source" | awk '{print $1}')
body_sha256=$(shasum -a 256 "$probe_output" | awk '{print $1}')
cat "$probe_output"
printf 'stemsbeamstumpcorpussummary pages %d systems %d constructors %d sides %d neighbors %d seedInputs %d purgeComparisons %d purgeRemovals %d purgeBreaks %d sideSeeds %d buildAttempts %d emptySections %d zeroCompounds %d candidates %d directionAccepted %d directionRejected %d registrations %d newBuilds %d reusedBuilds %d sectionRows %d steps %d finalStumps %d finalSideStumps %d tremolos %d probeSourceSha256 %s runnerSourceSha256 %s emittedBodySha256 %s\n' \
    "$pages" "$systems" "$constructors" "$sides" "$neighbors" "$seed_inputs" \
    "$purge_comparisons" "$purge_removals" "$purge_breaks" "$side_seeds" \
    "$build_attempts" "$empty_sections" "$zero_compounds" "$candidates" \
    "$direction_accepted" "$direction_rejected" "$registrations" "$new_builds" \
    "$reused_builds" "$section_rows" "$steps" "$final_stumps" "$final_side_stumps" \
    "$tremolos" "$probe_source_sha256" "$runner_source_sha256" "$body_sha256"
