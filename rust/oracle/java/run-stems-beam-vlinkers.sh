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
probe_source="$repo_root/rust/oracle/java/StemsBeamVLinkerProbe.java"
runner_source="$repo_root/rust/oracle/java/run-stems-beam-vlinkers.sh"

if [ ! -f "$probe_cp_file" ]; then
    echo "missing saved app runtime classpath: $probe_cp_file" >&2
    exit 2
fi

probe_classes=$(mktemp -d /private/tmp/stems-beam-vlinkers-classes.XXXXXX)
probe_output=$(mktemp /private/tmp/stems-beam-vlinkers-output.XXXXXX)
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
            org.audiveris.omr.rustport.StemsBeamVLinkerProbe "$target"
    )
}

{
    env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
        -XX:+UnlockExperimentalVMOptions \
        -XX:+UseEpsilonGC \
        -Xmx4g \
        -cp "$probe_classes:$probe_cp" \
        org.audiveris.omr.rustport.StemsBeamVLinkerProbe --header

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
    /^stemsbeamvlinkerpagesummary / {
        pages++
        for (key in fields) delete fields[key]
        for (key in seen) delete seen[key]
        for (i = 3; i <= NF; i += 2) {
            key = $i
            if (seen[key]++) duplicates++
            fields[key] = $(i + 1)
        }
        required = "systems constructors survivors tremolos parts stumpBs orphanAttempts " \
                "orphanCreated Bs Vs stumpVs initialGeometries rebuiltGeometries " \
                "alienNeighborScans alienCandidates alienSurvivors alienLimiters seedChecks " \
                "reachableSeeds orphanBs zeroDirectionBs orphanVs topVs bottomVs sideBLinks " \
                "orphanChecks orphanExisting orphanEmpty orphanInterior partFoldRows " \
                "finalGeometries geometryRows alienLookups alienCandidateRows " \
                "alienGroupDrops alienBadDrops " \
                "alienHookDrops alienMissDrops alienAlignedDrops alienAcceptedRows alienSortRows " \
                "alienShrinkRows seedCandidateRows seedHitRows hash"
        count = split(required, names, " ")
        for (i = 1; i <= count; i++) {
            if (!(names[i] in fields)) missing++
        }
        systems += fields["systems"]
        constructors += fields["constructors"]
        survivors += fields["survivors"]
        tremolos += fields["tremolos"]
        parts += fields["parts"]
        stumpBs += fields["stumpBs"]
        orphanAttempts += fields["orphanAttempts"]
        orphanCreated += fields["orphanCreated"]
        Bs += fields["Bs"]
        Vs += fields["Vs"]
        stumpVs += fields["stumpVs"]
        initialGeometries += fields["initialGeometries"]
        rebuiltGeometries += fields["rebuiltGeometries"]
        alienNeighborScans += fields["alienNeighborScans"]
        alienCandidates += fields["alienCandidates"]
        alienSurvivors += fields["alienSurvivors"]
        alienLimiters += fields["alienLimiters"]
        seedChecks += fields["seedChecks"]
        reachableSeeds += fields["reachableSeeds"]
        orphanBs += fields["orphanBs"]
        zeroDirectionBs += fields["zeroDirectionBs"]
        orphanVs += fields["orphanVs"]
        topVs += fields["topVs"]
        bottomVs += fields["bottomVs"]
        sideBLinks += fields["sideBLinks"]
        orphanChecks += fields["orphanChecks"]
        orphanExisting += fields["orphanExisting"]
        orphanEmpty += fields["orphanEmpty"]
        orphanInterior += fields["orphanInterior"]
        partFoldRows += fields["partFoldRows"]
        finalGeometries += fields["finalGeometries"]
        geometryRows += fields["geometryRows"]
        alienLookups += fields["alienLookups"]
        alienCandidateRows += fields["alienCandidateRows"]
        alienGroupDrops += fields["alienGroupDrops"]
        alienBadDrops += fields["alienBadDrops"]
        alienHookDrops += fields["alienHookDrops"]
        alienMissDrops += fields["alienMissDrops"]
        alienAlignedDrops += fields["alienAlignedDrops"]
        alienAcceptedRows += fields["alienAcceptedRows"]
        alienSortRows += fields["alienSortRows"]
        alienShrinkRows += fields["alienShrinkRows"]
        seedCandidateRows += fields["seedCandidateRows"]
        seedHitRows += fields["seedHitRows"]
    }
    END {
        printf "%d", pages
        printf ":%d:%d:%d:%d:%d", systems, constructors, survivors, tremolos, parts
        printf ":%d:%d:%d:%d:%d:%d", stumpBs, orphanAttempts, orphanCreated, Bs, Vs, stumpVs
        printf ":%d:%d:%d:%d:%d:%d:%d:%d", initialGeometries, rebuiltGeometries, \
                alienNeighborScans, alienCandidates, alienSurvivors, alienLimiters, seedChecks, \
                reachableSeeds
        printf ":%d:%d:%d:%d:%d:%d", orphanBs, zeroDirectionBs, orphanVs, topVs, bottomVs, \
                sideBLinks
        printf ":%d:%d:%d:%d:%d", orphanChecks, orphanExisting, orphanEmpty, orphanInterior, \
                partFoldRows
        printf ":%d:%d:%d:%d", finalGeometries, geometryRows, alienLookups, alienCandidateRows
        printf ":%d:%d:%d:%d:%d:%d:%d:%d", alienGroupDrops, alienBadDrops, alienHookDrops, \
                alienMissDrops, alienAlignedDrops, alienAcceptedRows, alienSortRows, \
                alienShrinkRows
        printf ":%d:%d:%d:%d\n", seedCandidateRows, seedHitRows, missing, duplicates
    }
' "$probe_output")

IFS=: read -r pages systems constructors survivors tremolos parts stump_bs orphan_attempts \
    orphan_created b_linkers v_linkers stump_vs initial_geometries rebuilt_geometries \
    alien_neighbor_scans alien_candidates alien_survivors alien_limiters seed_checks \
    reachable_seeds orphan_bs zero_direction_bs orphan_vs top_vs bottom_vs side_b_links \
    orphan_checks orphan_existing orphan_empty orphan_interior part_fold_rows \
    final_geometries geometry_rows alien_lookups alien_candidate_rows alien_group_drops alien_bad_drops \
    alien_hook_drops alien_miss_drops alien_aligned_drops alien_accepted_rows alien_sort_rows \
    alien_shrink_rows seed_candidate_rows seed_hit_rows missing_fields duplicate_fields <<EOF
$summary_counts
EOF

if [ "$missing_fields:$duplicate_fields" != "0:0" ]; then
    echo "STEMS beam-VLinker page summaries have $missing_fields missing and $duplicate_fields duplicate fields" >&2
    exit 1
fi

row_counts=$(awk '
    /^stemsbeamvlinkerpage / { pages++ }
    /^stemsbeamvlinkersystem / { systems++ }
    /^stemsbeamvlinkerpart / { parts++ }
    /^stemsbeamvlinkerconstructor / { constructors++ }
    /^stemsbeamvlinkerb / { Bs++ }
    /^stemsbeamvlinkerorphan / { orphans++ }
    /^stemsbeamvlinkerv / { Vs++ }
    /^stemsbeamvlinkerseedcandidate / { seedCandidates++ }
    /^stemsbeamvlinkerseed / { seeds++ }
    /^stemsbeamvlinkerlimit / { limits++ }
    /^stemsbeamvlinkerlimitstaff / { limitStaff++ }
    /^stemsbeamvlinkeralienlookup / { alienLookups++ }
    /^stemsbeamvlinkeralien / { aliens++ }
    /^stemsbeamvlinkeraliensort / { alienSort++ }
    /^stemsbeamvlinkeralienselected / { alienSelected++ }
    /^stemsbeamvlinkergeometry / { geometries++ }
    /^stemsbeamvlinkersystemsummary / { systemSummaries++ }
    /^stemsbeamvlinkerpagesummary / { pageSummaries++ }
    END {
        printf "%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d\n", pages, \
                systems, parts, constructors, orphans, Bs, Vs, seedCandidates, seeds, limits, \
                limitStaff, alienLookups, aliens, alienSort, alienSelected, geometries, \
                systemSummaries, pageSummaries
    }
' "$probe_output")
IFS=: read -r row_pages row_systems row_parts row_constructors row_orphans row_bs row_vs \
    row_seed_candidates row_seeds row_limits row_limit_staff row_alien_lookups row_aliens \
    row_alien_sort row_alien_selected row_geometries row_system_summaries row_page_summaries <<EOF
$row_counts
EOF

if [ "$pages:$systems:$constructors" != "8:30:803" ]; then
    echo "unexpected STEMS beam-VLinker corpus shape $pages:$systems:$constructors" >&2
    exit 1
fi
if [ "$row_pages:$row_systems:$row_parts:$row_constructors:$row_orphans:$row_bs:$row_vs:$row_seed_candidates:$row_seeds:$row_limits:$row_limit_staff:$row_alien_lookups:$row_aliens:$row_alien_sort:$row_alien_selected:$row_geometries:$row_system_summaries:$row_page_summaries" != \
     "$pages:$systems:$parts:$constructors:$orphan_checks:$b_linkers:$v_linkers:$seed_checks:$reachable_seeds:$v_linkers:$part_fold_rows:$alien_lookups:$alien_candidate_rows:$alien_sort_rows:$alien_lookups:$geometry_rows:$systems:$pages" ]; then
    echo "STEMS beam-VLinker detail row counts disagree with page summaries" >&2
    exit 1
fi

if [ $((survivors + tremolos)) -ne "$constructors" ] \
        || [ "$orphan_checks" -ne $((2 * constructors)) ] \
        || [ "$orphan_attempts" -ne $((orphan_checks - orphan_existing)) ] \
        || [ "$orphan_checks" -ne $((orphan_existing + orphan_empty + orphan_interior + orphan_created)) ] \
        || [ "$side_b_links" -ne $((orphan_existing + orphan_created)) ] \
        || [ "$orphan_bs" -ne "$orphan_created" ] \
        || [ "$b_linkers" -ne $((stump_bs + orphan_bs)) ] \
        || [ "$orphan_vs" -ne $((2 * orphan_bs)) ] \
        || [ "$v_linkers" -ne $((stump_vs + orphan_vs)) ] \
        || [ "$v_linkers" -ne $((top_vs + bottom_vs)) ] \
        || [ "$initial_geometries" -ne "$v_linkers" ] \
        || [ "$final_geometries" -ne "$v_linkers" ] \
        || [ "$rebuilt_geometries" -ne "$alien_limiters" ] \
        || [ "$geometry_rows" -ne $((initial_geometries + final_geometries)) ] \
        || [ "$alien_lookups" -ne "$v_linkers" ] \
        || [ "$alien_neighbor_scans" -ne "$alien_candidates" ] \
        || [ "$alien_candidate_rows" -ne "$alien_candidates" ] \
        || [ "$alien_candidates" -ne $((alien_group_drops + alien_bad_drops + alien_hook_drops + alien_miss_drops + alien_aligned_drops + alien_accepted_rows)) ] \
        || [ "$alien_survivors" -ne "$alien_accepted_rows" ] \
        || [ "$alien_sort_rows" -ne "$alien_accepted_rows" ] \
        || [ "$alien_limiters" -ne "$alien_shrink_rows" ] \
        || [ "$seed_checks" -ne "$seed_candidate_rows" ] \
        || [ "$reachable_seeds" -ne "$seed_hit_rows" ] \
        || [ "$reachable_seeds" -gt "$seed_checks" ] \
        || [ "$zero_direction_bs" -gt "$stump_bs" ]; then
    echo "STEMS beam-VLinker corpus totals violate coverage invariants" >&2
    exit 1
fi

bad_systems=$(awk '
    /^stemsbeamvlinkersystemsummary / {
        for (key in fields) delete fields[key]
        for (key in seen) delete seen[key]
        duplicate = 0
        for (i = 5; i <= NF; i += 2) {
            key = $i
            if (seen[key]++) duplicate = 1
            fields[key] = $(i + 1)
        }
        required = "systems constructors survivors tremolos parts stumpBs orphanAttempts " \
                "orphanCreated Bs Vs stumpVs initialGeometries rebuiltGeometries " \
                "alienNeighborScans alienCandidates alienSurvivors alienLimiters seedChecks " \
                "reachableSeeds orphanBs zeroDirectionBs orphanVs topVs bottomVs sideBLinks " \
                "orphanChecks orphanExisting orphanEmpty orphanInterior partFoldRows " \
                "finalGeometries geometryRows alienLookups alienCandidateRows " \
                "alienGroupDrops alienBadDrops alienHookDrops alienMissDrops " \
                "alienAlignedDrops alienAcceptedRows alienSortRows alienShrinkRows " \
                "seedCandidateRows seedHitRows hash"
        count = split(required, names, " ")
        missing = 0
        for (i = 1; i <= count; i++) {
            if (!(names[i] in fields)) missing = 1
        }
        if (duplicate || missing || fields["systems"] != 1 ||
                fields["constructors"] != fields["survivors"] + fields["tremolos"] ||
                fields["orphanChecks"] != 2 * fields["constructors"] ||
                fields["orphanAttempts"] != fields["orphanChecks"] - fields["orphanExisting"] ||
                fields["orphanChecks"] != (fields["orphanExisting"] + fields["orphanEmpty"] + fields["orphanInterior"] + fields["orphanCreated"]) ||
                fields["sideBLinks"] != fields["orphanExisting"] + fields["orphanCreated"] ||
                fields["orphanBs"] != fields["orphanCreated"] ||
                fields["Bs"] != fields["stumpBs"] + fields["orphanBs"] ||
                fields["orphanVs"] != 2 * fields["orphanBs"] ||
                fields["Vs"] != fields["stumpVs"] + fields["orphanVs"] ||
                fields["Vs"] != fields["topVs"] + fields["bottomVs"] ||
                fields["initialGeometries"] != fields["Vs"] ||
                fields["finalGeometries"] != fields["Vs"] ||
                fields["rebuiltGeometries"] != fields["alienLimiters"] ||
                fields["geometryRows"] != (fields["initialGeometries"] + fields["finalGeometries"]) ||
                fields["alienLookups"] != fields["Vs"] ||
                fields["alienNeighborScans"] != fields["alienCandidates"] ||
                fields["alienCandidateRows"] != fields["alienCandidates"] ||
                fields["alienCandidates"] != (fields["alienGroupDrops"] + fields["alienBadDrops"] + fields["alienHookDrops"] + fields["alienMissDrops"] + fields["alienAlignedDrops"] + fields["alienAcceptedRows"]) ||
                fields["alienSurvivors"] != fields["alienAcceptedRows"] ||
                fields["alienSortRows"] != fields["alienAcceptedRows"] ||
                fields["alienLimiters"] != fields["alienShrinkRows"] ||
                fields["seedChecks"] != fields["seedCandidateRows"] ||
                fields["reachableSeeds"] != fields["seedHitRows"] ||
                fields["reachableSeeds"] > fields["seedChecks"] ||
                fields["zeroDirectionBs"] > fields["stumpBs"]) bad++
    }
    END { print bad + 0 }
' "$probe_output")
if [ "$bad_systems" -ne 0 ]; then
    echo "$bad_systems system summaries violate beam-VLinker invariants" >&2
    exit 1
fi

observed_totals="$constructors:$survivors:$tremolos:$parts:$stump_bs:$orphan_attempts:$orphan_created:$b_linkers:$v_linkers:$stump_vs:$initial_geometries:$rebuilt_geometries:$alien_neighbor_scans:$alien_candidates:$alien_survivors:$alien_limiters:$seed_checks:$reachable_seeds:$orphan_bs:$zero_direction_bs:$orphan_vs:$top_vs:$bottom_vs:$side_b_links:$orphan_checks:$orphan_existing:$orphan_empty:$orphan_interior:$part_fold_rows:$final_geometries:$geometry_rows:$alien_lookups:$alien_candidate_rows:$alien_group_drops:$alien_bad_drops:$alien_hook_drops:$alien_miss_drops:$alien_aligned_drops:$alien_accepted_rows:$alien_sort_rows:$alien_shrink_rows:$seed_candidate_rows:$seed_hit_rows"
expected_totals="803:803:0:30:1821:295:295:2116:2417:1827:2417:703:9186:9186:1094:703:12491:2169:295:0:590:1389:1028:1606:1606:1311:0:0:2860:2417:4834:2417:9186:4738:38:501:2812:3:1094:1094:703:12491:2169"
if [ "$observed_totals" != "$expected_totals" ]; then
    echo "unexpected STEMS beam-VLinker corpus totals" >&2
    echo "expected: $expected_totals" >&2
    echo "observed: $observed_totals" >&2
    exit 1
fi
probe_source_sha256=$(shasum -a 256 "$probe_source" | awk '{print $1}')
runner_source_sha256=$(shasum -a 256 "$runner_source" | awk '{print $1}')
body_sha256=$(shasum -a 256 "$probe_output" | awk '{print $1}')
cat "$probe_output"
printf 'stemsbeamvlinkercorpussummary pages %d systems %d constructors %d survivors %d tremolos %d parts %d stumpBs %d orphanAttempts %d orphanCreated %d Bs %d Vs %d stumpVs %d initialGeometries %d rebuiltGeometries %d alienNeighborScans %d alienCandidates %d alienSurvivors %d alienLimiters %d seedChecks %d reachableSeeds %d orphanBs %d zeroDirectionBs %d orphanVs %d topVs %d bottomVs %d sideBLinks %d orphanChecks %d orphanExisting %d orphanEmpty %d orphanInterior %d partFoldRows %d finalGeometries %d geometryRows %d alienLookups %d alienCandidateRows %d alienGroupDrops %d alienBadDrops %d alienHookDrops %d alienMissDrops %d alienAlignedDrops %d alienAcceptedRows %d alienSortRows %d alienShrinkRows %d seedCandidateRows %d seedHitRows %d probeSourceSha256 %s runnerSourceSha256 %s emittedBodySha256 %s\n' \
    "$pages" "$systems" "$constructors" "$survivors" "$tremolos" "$parts" \
    "$stump_bs" "$orphan_attempts" "$orphan_created" "$b_linkers" "$v_linkers" \
    "$stump_vs" "$initial_geometries" "$rebuilt_geometries" "$alien_neighbor_scans" \
    "$alien_candidates" "$alien_survivors" "$alien_limiters" "$seed_checks" \
    "$reachable_seeds" "$orphan_bs" "$zero_direction_bs" "$orphan_vs" "$top_vs" \
    "$bottom_vs" "$side_b_links" "$orphan_checks" "$orphan_existing" "$orphan_empty" \
    "$orphan_interior" "$part_fold_rows" "$final_geometries" "$geometry_rows" \
    "$alien_lookups" "$alien_candidate_rows" "$alien_group_drops" "$alien_bad_drops" "$alien_hook_drops" \
    "$alien_miss_drops" "$alien_aligned_drops" "$alien_accepted_rows" "$alien_sort_rows" \
    "$alien_shrink_rows" "$seed_candidate_rows" "$seed_hit_rows" "$probe_source_sha256" \
    "$runner_source_sha256" "$body_sha256"
