#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze Java Bach system-2 phase-two queue 14's shared-stump RIGHT/BOTH retry.
set -eu

if [ -z "${JAVA_HOME:-}" ] || [ ! -x "$JAVA_HOME/bin/java" ]; then
    echo "JAVA_HOME must name the frozen Temurin JDK 25" >&2
    exit 2
fi
release_field()
{
    awk -F= -v name="$1" '$1 == name { value = $2; gsub(/^"|"$/, "", value); print value }' \
        "$JAVA_HOME/release"
}
if [ "$(release_field IMPLEMENTOR)" != "Eclipse Adoptium" ] || \
        [ "$(release_field IMPLEMENTOR_VERSION)" != "Temurin-25.0.3+9" ] || \
        [ "$(release_field JAVA_RUNTIME_VERSION)" != "25.0.3+9-LTS" ] || \
        [ "$(release_field OS_NAME)" != "Darwin" ] || \
        [ "$(release_field OS_ARCH)" != "aarch64" ] || \
        [ "$(release_field JVM_VARIANT)" != "Hotspot" ]; then
    echo "JAVA_HOME is not frozen Temurin 25.0.3+9-LTS aarch64 HotSpot" >&2
    exit 2
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-two-bach-s2-q14.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
probe="$script_dir/StemsHeadPhaseTwoPageProbe.java"
init="$script_dir/stems-head-phase-two-page.init.gradle"
input="$repo_root/data/examples/BachInvention5.jpg"
stems_source="$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/StemsRetriever.java"

run_pass()
{
    target=$1
    (
        cd "$repo_root"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
            JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon -q \
            -PrustPortRepo="$repo_root" -PphaseTwoPage="$input" \
            -I "$init" :app:stemsHeadPhaseTwoPageProbe
    ) > "$target"
}

run_pass "$tmp_dir/warmup"
run_pass "$tmp_dir/pass1"
run_pass "$tmp_dir/pass2"
grep -E '^(stemsheadphase2baseline .* system 2 |stemsheadphase2retry .* system 2 queueIndex 14 )' \
    "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep -E '^(stemsheadphase2baseline .* system 2 |stemsheadphase2retry .* system 2 queueIndex 14 )' \
    "$tmp_dir/pass2" > "$tmp_dir/rows2"
if ! cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2"; then
    echo "fresh Bach system-2 phase-two queue-14 Java passes are not byte-identical" >&2
    diff "$tmp_dir/rows1" "$tmp_dir/rows2" | head -12 >&2
    exit 1
fi

if [ "$(wc -l < "$tmp_dir/rows1" | tr -d ' ')" -ne 2 ] || \
        ! grep -q '^stemsheadphase2baseline page BachInvention5.jpg#1 system 2 heads 215 queueSize 15 queue \[x185:sig213:id4034,x159:sig164:id3939,x194:sig78:id3761,x163:sig170:id3951,x160:sig169:id3949,x162:sig168:id3947,x158:sig88:id3781,x152:sig90:id3784,x123:sig14:id3633,x149:sig18:id3641,x190:sig214:id4036,x98:sig136:id3878,x30:sig95:id3796,x118:sig211:id4031,x54:sig59:id3723\] sigVertices 394 sigEdges 600 systemStems 77 allocator 6815$' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2retry page BachInvention5.jpg#1 system 2 queueIndex 14 headX 54 headSig 59 headInterId 3723 grade 3fc57085228ee157 append true sidesBefore \[LEFT:false:true,RIGHT:false:true\] decisions \[LEFT:top=false:bottom=false:branch=Neither,RIGHT:top=false:bottom=false:branch=Neither\] returned false sidesAfter \[LEFT:false:true,RIGHT:false:true\] undefs \[\] sideChanges \[\] sigVerticesBefore 394 sigVerticesAfter 394 sigEdgesBefore 602 sigEdgesAfter 602 systemStemsBefore 77 systemStemsAfter 77 allocatorBefore 6815 allocatorAfter 6815$' "$tmp_dir/rows1"; then
    echo "Bach system-2 phase-two queue-14 Java contract differs" >&2
    cat "$tmp_dir/rows1" >&2
    exit 1
fi

base_runner="$script_dir/run-stems-head-phase-two-bach-system2-order13.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-two-bach-system2-order13.txt"
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
if [ "$base_runner_sha" != "9b4548be4751f2d3f97c3a09bf5d6de600e9b0d45f6e6ddba0a607b0725316a7" ] || \
        [ "$base_fixture_sha" != "ae0058af20fb67d3ee6a4349a012462c0a38578870d6dfc5ad7813c3161698c0" ]; then
    echo "Bach system-2 phase-two queue-13 predecessor drifted" >&2
    exit 1
fi

input_sha=$(shasum -a 256 "$input" | awk '{print $1}')
stems_source_sha=$(shasum -a 256 "$stems_source" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
if [ "$input_sha" != "9ab9a9a8ebf609d60a7d0cddcccd5ffc91d433812fd86153244595371d282805" ] || \
        [ "$stems_source_sha" != "26e95fa09905b39ea0dcae2b65a85b4e4fcb49b772c57f97f332a00c4dc8b9e7" ] || \
        [ "$probe_sha" != "7b467c57b65e57aa052296164129ae8c016d82756c9f804d8e1072747b0a76b2" ] || \
        [ "$init_sha" != "1defbc545668eb711395283bc0d8f9216b7402ad3b0f2f64f93812ac739c495e" ]; then
    echo "Bach phase-two input or Java evidence source drifted" >&2
    exit 1
fi

runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$tmp_dir/rows1" | awk '{print $1}')
out="$repo_root/rust/oracle/stems-head-phase-two-bach-system2-order14.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Bach system-2 phase-two queue 14.'
    echo '# schema: stems-head-phase-two-bach-system2-order14-v1'
    cat "$tmp_dir/rows1"
    printf '%s\n' \
        "stemsheadphase2bachs2q14summary schema stems-head-phase-two-bach-system2-order14-v1 page BachInvention5.jpg#1 system 2 rows 2 inputSha256 $input_sha stemsRetrieverSourceSha256 $stems_source_sha probeSourceSha256 $probe_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha baseBoundary255RunnerSha256 $base_runner_sha baseBoundary255FixtureSha256 $base_fixture_sha emittedBodySha256 $body_sha semanticPassSha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope BachSystem2PhaseTwoOrder14NoLinkRetry javaEvidence ReturnedAfterSystem2RetryIndex14"
} > "$out"
echo "wrote $out"
