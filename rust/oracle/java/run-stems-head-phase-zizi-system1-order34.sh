#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Deterministic Java evidence for Zizi system-1 head order 34.
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-zizi-order34.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
base_probe="$script_dir/StemsHeadPhaseTwoPageProbe.java"
transform="$script_dir/stems-head-phase-zizi-order34.transform.awk"
probe="$tmp_dir/StemsHeadPhaseTwoPageProbe.java"
init="$script_dir/stems-head-phase-zizi-order34.init.gradle"
input="$repo_root/data/examples/zizi.png"
stems_source="$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/StemsRetriever.java"
base_runner="$script_dir/run-stems-finalize-allegretto.sh"
base_fixture="$repo_root/rust/oracle/stems-finalize-allegretto-v1.txt"

awk -f "$transform" "$base_probe" > "$probe"

run_pass()
{
    target=$1
    (
        cd "$repo_root"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
            JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon -q \
            -PziziOrder34ProbeSource="$probe" -I "$init" \
            :app:stemsHeadZiziOrder34Probe
    ) > "$target"
}

run_pass "$tmp_dir/warmup"
run_pass "$tmp_dir/pass1"
run_pass "$tmp_dir/pass2"
grep '^stemsheadziziorder34 ' "$tmp_dir/pass1" > "$tmp_dir/row1"
grep '^stemsheadziziorder34 ' "$tmp_dir/pass2" > "$tmp_dir/row2"
if [ "$(wc -l < "$tmp_dir/row1" | tr -d ' ')" -ne 1 ] || \
        ! cmp -s "$tmp_dir/row1" "$tmp_dir/row2"; then
    echo "Zizi order-34 Java row is absent or not byte-identical" >&2
    exit 1
fi

input_sha=$(shasum -a 256 "$input" | awk '{print $1}')
stems_source_sha=$(shasum -a 256 "$stems_source" | awk '{print $1}')
base_probe_sha=$(shasum -a 256 "$base_probe" | awk '{print $1}')
transform_sha=$(shasum -a 256 "$transform" | awk '{print $1}')
probe_sha=$(shasum -a 256 "$probe" | awk '{print $1}')
init_sha=$(shasum -a 256 "$init" | awk '{print $1}')
runner_sha=$(shasum -a 256 "$0" | awk '{print $1}')
base_runner_sha=$(shasum -a 256 "$base_runner" | awk '{print $1}')
base_fixture_sha=$(shasum -a 256 "$base_fixture" | awk '{print $1}')
body_sha=$(shasum -a 256 "$tmp_dir/row1" | awk '{print $1}')
if [ "$input_sha" != "f6c613b3a60423dadde60d5e61ee7c1a641eef71c9fc6b6e8bdf5fab4c3c3e94" ] || \
        [ "$stems_source_sha" != "26e95fa09905b39ea0dcae2b65a85b4e4fcb49b772c57f97f332a00c4dc8b9e7" ] || \
        [ "$base_runner_sha" != "abafa7d183ae151baa7ed4d8005257c562e0c49fb939fe931a7571994d70d890" ] || \
        [ "$base_fixture_sha" != "cfb9e6011ed29aa30e6e90db6eeae931a3a6533d7339d80519a5ddd650c0ff0c" ]; then
    echo "Zizi source or Boundary-183 predecessor drifted" >&2
    exit 1
fi

out="$repo_root/rust/oracle/stems-head-phase-zizi-system1-order34.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Zizi head order 34.'
    echo '# schema: stems-head-phase-zizi-system1-order34-v1'
    cat "$tmp_dir/row1"
    printf '%s\n' \
        "stemsheadziziorder34summary schema stems-head-phase-zizi-system1-order34-v1 page zizi.png#1 system 1 rows 1 inputSha256 $input_sha stemsRetrieverSourceSha256 $stems_source_sha baseProbeSourceSha256 $base_probe_sha transformSourceSha256 $transform_sha transformedProbeSourceSha256 $probe_sha initSourceSha256 $init_sha runnerSourceSha256 $runner_sha baseFinalizeRunnerSha256 $base_runner_sha baseFinalizeFixtureSha256 $base_fixture_sha emittedBodySha256 $body_sha freshRuns 2 freshRunsByteIdentical true nativeScope ZiziSystem1Order34DuplicateIdempotentClosure javaEvidence ReturnedBeforeThirtySixthHead"
} > "$out"
echo "wrote $out"
