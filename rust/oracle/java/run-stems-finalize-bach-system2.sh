#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze Java Bach system-2 Java finalizeStems's shared-stump RIGHT/BOTH retry.
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
tmp_dir=$(mktemp -d /private/tmp/stems-finalize-bach-s2.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
probe="$script_dir/StemsHeadFinalizePageProbe.java"
init="$script_dir/stems-head-finalize-page.init.gradle"
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
            -I "$init" :app:stemsHeadFinalizePageProbe
    ) > "$target"
}

run_pass "$tmp_dir/warmup"
run_pass "$tmp_dir/pass1"
run_pass "$tmp_dir/pass2"
grep -E '^stemsheadfinalize page BachInvention5.jpg#1 system 2 ' \
    "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep -E '^stemsheadfinalize page BachInvention5.jpg#1 system 2 ' \
    "$tmp_dir/pass2" > "$tmp_dir/rows2"
if ! cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2"; then
    echo "fresh Bach system-2 phase-two Java finalizer Java passes are not byte-identical" >&2
    diff "$tmp_dir/rows1" "$tmp_dir/rows2" | head -12 >&2
    exit 1
fi

if [ "$(wc -l < "$tmp_dir/rows1" | tr -d ' ')" -ne 1 ] || \
        ! grep -q '^stemsheadfinalize page BachInvention5.jpg#1 system 2 checked 215 multipleBefore 1 multipleAfter 0 noStem 12 abnormal 12 removed 1 abnormalChanges 0 sigEdges 601 systemStems 77 allocator 6815$' "$tmp_dir/rows1"; then
    echo "Bach system-2 phase-two Java finalizer Java contract differs" >&2
    cat "$tmp_dir/rows1" >&2
    exit 1
fi

base_runner="$script_dir/run-stems-head-phase-two-bach-system2-order14.sh"
base_fixture="$repo_root/rust/oracle/stems-head-phase-two-bach-system2-order14.txt"
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
if [ "$base_runner_sha" != "d711efae48f0a8ca434936b7e68ac143cee9593867c63a853c90f28b330d3549" ] || \
        [ "$base_fixture_sha" != "f83e26e02df6ba19f58ab48742ee4f53b1341a2064c3d3151d16aa9598b1ae43" ]; then
    echo "Bach system-2 phase-two queue-14 predecessor drifted" >&2
    exit 1
fi

input_sha=$(shasum -a 256 "$input" | awk '{print $1}')
stems_source_sha=$(shasum -a 256 "$stems_source" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
if [ "$input_sha" != "9ab9a9a8ebf609d60a7d0cddcccd5ffc91d433812fd86153244595371d282805" ] || \
        [ "$stems_source_sha" != "26e95fa09905b39ea0dcae2b65a85b4e4fcb49b772c57f97f332a00c4dc8b9e7" ] || \
        [ "$probe_sha" != "07240ff53e6efeed338378fbec91b90ba2b3645540774fac3871be283805f76c" ] || \
        [ "$init_sha" != "a52be045074829368e68fadcdcabc2a1ee59ff0d427350a26cf7853d1cbd7250" ]; then
    echo "Bach phase-two input or Java evidence source drifted" >&2
    exit 1
fi

runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
body_sha=$(shasum -a 256 "$tmp_dir/rows1" | awk '{print $1}')
out="$repo_root/rust/oracle/stems-finalize-bach-system2.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Bach system-2 Java finalizeStems.'
    echo '# schema: stems-finalize-bach-system2-v1'
    cat "$tmp_dir/rows1"
    printf '%s\n' \
        "stemsfinalizebachs2summary schema stems-finalize-bach-system2-v1 page BachInvention5.jpg#1 system 2 rows 1 inputSha256 $input_sha stemsRetrieverSourceSha256 $stems_source_sha probeSourceSha256 $probe_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha baseBoundary256RunnerSha256 $base_runner_sha baseBoundary256FixtureSha256 $base_fixture_sha emittedBodySha256 $body_sha semanticPassSha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope BachSystem2FinalizeStemsCensus javaEvidence ReturnedAfterFinalizeStemsBachSystem2"
} > "$out"
echo "wrote $out"
