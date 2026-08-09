#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
set -eu

mode=chula
if [ "$#" -gt 1 ]; then
    echo "usage: $0 [--full]" >&2
    exit 2
fi
if [ "$#" -eq 1 ]; then
    if [ "$1" != "--full" ]; then
        echo "usage: $0 [--full]" >&2
        exit 2
    fi
    mode=full
fi

if [ -z "${JAVA_HOME:-}" ]; then
    echo "JAVA_HOME must name the frozen Temurin JDK 25" >&2
    exit 2
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
probe_cp_file=${AUDIVERIS_PROBE_CLASSPATH_FILE:-/private/tmp/audiveris-probe.classpath}
probe_source="$repo_root/rust/oracle/java/StemsBeamExpandProbe.java"
runner_source="$repo_root/rust/oracle/java/run-stems-beam-expand.sh"

if [ ! -f "$probe_cp_file" ]; then
    echo "missing saved app runtime classpath: $probe_cp_file" >&2
    exit 2
fi

probe_source_sha256=$(shasum -a 256 "$probe_source" | awk '{print $1}')
expected_probe_source_sha256=2a5e107f947e140e030f3cc1dff06105ab730af3e41381e76f5f8113a17b0fa2
if [ "$probe_source_sha256" != "$expected_probe_source_sha256" ]; then
    echo "unexpected beam-expand probe source fingerprint" >&2
    echo "observed: $probe_source_sha256" >&2
    exit 1
fi

probe_classes=$(mktemp -d /private/tmp/stems-beam-expand-classes.XXXXXX)
probe_output=$(mktemp /private/tmp/stems-beam-expand-output.XXXXXX)
trap 'rm -rf "$probe_classes"; rm -f "$probe_output"' EXIT HUP INT TERM
probe_cp=$(sed -n '1p' "$probe_cp_file")

"$JAVA_HOME/bin/javac" \
    -cp "$probe_cp" \
    -d "$probe_classes" \
    "$probe_source"

run_probe ()
{
    target=$1
    if [ "$mode" = full ]; then
        # The profile matrix allocates enough transient replay evidence for the
        # large Bach scan to exhaust a 48 GiB Epsilon heap.  Full exploration
        # therefore uses the JDK default collector; frozen one-page runs retain
        # Epsilon for the strictest deterministic checkpoint.
        (
            cd "$repo_root/app"
            env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
                -Xmx48g \
                -Djava.awt.headless=true \
                -Dlogback.configurationFile="$repo_root/rust/oracle/java/logback-quiet.xml" \
                -cp "$probe_classes:$probe_cp" \
                org.audiveris.omr.rustport.StemsBeamExpandProbe "$target"
        )
    else
        (
            cd "$repo_root/app"
            env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
                -XX:+UnlockExperimentalVMOptions \
                -XX:+UseEpsilonGC \
                -Xmx48g \
                -Djava.awt.headless=true \
                -Dlogback.configurationFile="$repo_root/rust/oracle/java/logback-quiet.xml" \
                -cp "$probe_classes:$probe_cp" \
                org.audiveris.omr.rustport.StemsBeamExpandProbe "$target"
        )
    fi
}

{
    env -u JAVA_TOOL_OPTIONS "$JAVA_HOME/bin/java" \
        -XX:+UnlockExperimentalVMOptions \
        -XX:+UseEpsilonGC \
        -Xmx48g \
        -cp "$probe_classes:$probe_cp" \
        org.audiveris.omr.rustport.StemsBeamExpandProbe --header

    run_probe "$repo_root/data/examples/chula.png:1"
    if [ "$mode" = full ]; then
        run_probe "$repo_root/data/examples/allegretto.png:1"
        run_probe "$repo_root/data/examples/batuque.png:1"
        run_probe "$repo_root/data/examples/carmen.png:1"
        run_probe "$repo_root/data/examples/cucaracha.png:1"
        run_probe "$repo_root/data/examples/hove.png:1"
        run_probe "$repo_root/data/examples/zizi.png:1"
        run_probe "$repo_root/data/examples/BachInvention5.jpg:1"
    fi
} > "$probe_output"

schema_count=$(awk '$0 == "# schema: stems-beam-expand-v1" { count++ } END { print count + 0 }' \
    "$probe_output")
if [ "$schema_count" -ne 1 ]; then
    echo "beam-expand schema header is missing or duplicated" >&2
    exit 1
fi

row_counts=$(awk '
    /^stemsbeamexpandpage / { pages++ }
    /^stemsbeamexpandsystem / { systems++ }
    /^stemsbeamexpandplan / { plans++ }
    /^stemsbeamexpandgap / { gaps++ }
    /^stemsbeamexpandseparation / { separations++ }
    /^stemsbeamexpandrelation / { relationAttempts++ }
    /^stemsbeamexpandupdate / { updates++ }
    /^stemsbeamexpandfinalrelation / { finalRelations++ }
    /^stemsbeamexpandglyph / { glyphs++ }
    /^stemsbeamexpandend / { ends++ }
    /^stemsbeamexpandsystemsummary / { systemSummaries++ }
    /^stemsbeamexpandpagesummary / { pageSummaries++ }
    END {
        printf "%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d:%d\n",
                pages, systems, plans, gaps, separations, relationAttempts, updates,
                finalRelations, glyphs, ends, systemSummaries, pageSummaries
    }
' "$probe_output")

bad_pages=$(awk '
    /^stemsbeamexpandpagesummary / {
        for (key in f) delete f[key]
        for (key in seen) delete seen[key]
        duplicate = 0
        # Fields 3-4 are the page envelope system count.  The Totals payload
        # begins at field 5 and intentionally repeats the systems pair.
        for (i = 5; i <= NF; i += 2) {
            if (seen[$i]++) duplicate = 1
            f[$i] = $(i + 1)
        }
        bad = duplicate
        if ($3 != "systems" || $4 != f["systems"]) bad = 1
        if (f["plans"] != f["noHeadTarget"] + f["expandFailed"] + f["noRelations"] + f["noGlyphs"] + f["ready"]) bad = 1
        if (f["beamSideReady"] != f["beamSideReadyWithoutStoppingHead"] + f["beamSideReadyBeyondStoppingHead"] + f["beamSideReadyAtStoppingHead"]) bad = 1
        if (f["storedTheoMutations"] != f["attachmentLineMutations"]) bad = 1
        if (f["sigMutations"] + f["systemStemMutations"] + f["glyphIndexMutations"] + f["filamentIndexMutations"] + f["linkMutations"] + f["builderMutations"] != 0) bad = 1
        if (bad) badPages++
        plans += f["plans"]
        relations += f["relations"]
        glyphs += f["glyphs"]
    }
    /^stemsbeamexpandend / { endRows++ }
    /^stemsbeamexpandfinalrelation / { relationRows++ }
    /^stemsbeamexpandglyph / { glyphRows++ }
    END {
        if (plans != endRows || relations != relationRows || glyphs != glyphRows) badPages++
        print badPages + 0
    }
' "$probe_output")
if [ "$bad_pages" -ne 0 ]; then
    echo "$bad_pages beam-expand page summaries violate schema invariants" >&2
    exit 1
fi

body_sha256=$(shasum -a 256 "$probe_output" | awk '{print $1}')
body_lines=$(wc -l < "$probe_output" | tr -d ' ')
body_bytes=$(wc -c < "$probe_output" | tr -d ' ')
page_count=$(awk '/^stemsbeamexpandpage / { count++ } END { print count + 0 }' "$probe_output")
page_refs=$(awk '
    /^stemsbeamexpandpage / {
        if (count++) printf ","
        printf "%s", $2
    }
' "$probe_output")

if [ "$mode" = chula ]; then
    expected_rows=1:3:1735:125:1026:2038:4585:2034:1478:1735:3:1
    if [ "$row_counts" != "$expected_rows" ]; then
        echo "unexpected Chula beam-expand detail row counts" >&2
        echo "observed: $row_counts" >&2
        exit 1
    fi
    expected_page_summary='stemsbeamexpandpagesummary chula.png#1 systems 3 systems 3 builders 354 plans 1735 noHeadTarget 625 expandFailed 100 noRelations 2 noGlyphs 14 ready 994 relations 2034 glyphs 1478 postReturnRelations 0 plansWithPostReturnRelations 0 rollbackLineDivergences 0 relationSideMismatches 2 storedTheoMutations 161 attachmentLineMutations 161 beamSideReady 178 beamSideReadyWithoutStoppingHead 9 beamSideReadyBeyondStoppingHead 82 beamSideReadyAtStoppingHead 87 maxAbsStoredTheoShift 0x1.0f8e60e2a3f8p3/4020f8e60e2a3f80 sigMutations 0 systemStemMutations 0 glyphIndexMutations 0 filamentIndexMutations 0 linkMutations 0 builderMutations 0 hash 2f436688d138874d'
    observed_page_summary=$(awk '/^stemsbeamexpandpagesummary / { print }' "$probe_output")
    if [ "$observed_page_summary" != "$expected_page_summary" ]; then
        echo "unexpected Chula beam-expand page summary" >&2
        echo "observed: $observed_page_summary" >&2
        exit 1
    fi
    expected_body_sha256=ec622657c466029d5e696727df60d09a02f48c4349583a18c6fa1c65573986fc
    if [ "$body_sha256:$body_lines:$body_bytes" != \
         "$expected_body_sha256:14774:12629914" ]; then
        echo "unexpected Chula beam-expand body fingerprint" >&2
        echo "observed: $body_sha256:$body_lines:$body_bytes" >&2
        exit 1
    fi
else
    expected_rows=8:30:11573:578:9869:18416:37683:18345:12523:11573:30:8
    if [ "$row_counts" != "$expected_rows" ]; then
        echo "unexpected full beam-expand detail row counts" >&2
        echo "observed: $row_counts" >&2
        exit 1
    fi
    page_summaries_sha256=$(awk '/^stemsbeamexpandpagesummary / { print }' \
        "$probe_output" | shasum -a 256 | awk '{print $1}')
    expected_page_summaries_sha256=df1796b2e584556ef1fd51a56003bc2f5d95b3f4e12d412b8f71a62f63bec099
    if [ "$page_summaries_sha256" != "$expected_page_summaries_sha256" ]; then
        echo "unexpected full beam-expand page summaries" >&2
        echo "observed summary fingerprint: $page_summaries_sha256" >&2
        exit 1
    fi
    system_summaries_sha256=$(awk '/^stemsbeamexpandsystemsummary / { print }' \
        "$probe_output" | shasum -a 256 | awk '{print $1}')
    expected_system_summaries_sha256=64ae52a8e25d35f99c7c7c10792fdd466480393da627768edd9cab7febbf4010
    if [ "$system_summaries_sha256" != "$expected_system_summaries_sha256" ]; then
        echo "unexpected full beam-expand system summaries" >&2
        echo "observed summary fingerprint: $system_summaries_sha256" >&2
        exit 1
    fi
    expected_body_sha256=ac0fcb9880dbf720c8b73e6baf02867d05e0f2d5a62f208f52e9fa7d5c764966
    if [ "$body_sha256:$body_lines:$body_bytes" != \
         "$expected_body_sha256:120646:104048204" ]; then
        echo "unexpected full beam-expand body fingerprint" >&2
        echo "observed: $body_sha256:$body_lines:$body_bytes" >&2
        exit 1
    fi
fi

runner_source_sha256=$(shasum -a 256 "$runner_source" | awk '{print $1}')
cat "$probe_output"
printf 'stemsbeamexpandcorpussummary schema stems-beam-expand-v1 mode %s pages %s pageRefs %s rowCounts %s probeSourceSha256 %s runnerSourceSha256 %s emittedBodySha256 %s emittedBodyLines %s emittedBodyBytes %s\n' \
    "$mode" "$page_count" "$page_refs" "$row_counts" "$probe_source_sha256" "$runner_source_sha256" \
    "$body_sha256" "$body_lines" "$body_bytes"
