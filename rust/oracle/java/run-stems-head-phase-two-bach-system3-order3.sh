#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze Bach system 3 queue 3's RIGHT/BOTTOM reused-stem append.
set -eu

if [ -z "${JAVA_HOME:-}" ] || [ ! -x "$JAVA_HOME/bin/java" ]; then
    echo "JAVA_HOME must name the frozen Temurin JDK 25" >&2
    exit 2
fi
release_field() {
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
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-two-bach-s3-q3.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
base_probe="$script_dir/StemsHeadPhaseTwoPageProbe.java"
base_head_linker="$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/HeadLinker.java"
base_transform="$script_dir/stems-head-phase-two-x14.transform.awk"
retarget_transform="$script_dir/stems-head-phase-two-bach-system3-order3.transform.awk"
init="$script_dir/stems-head-phase-two-x14.init.gradle"
input="$repo_root/data/examples/BachInvention5.jpg"
probe="$tmp_dir/StemsHeadPhaseTwoPageProbe.java"
head_linker_x14="$tmp_dir/HeadLinker-x14.java"
head_linker_dir="$tmp_dir/head-linker"
head_linker="$head_linker_dir/HeadLinker.java"
mkdir "$head_linker_dir"
cp "$base_probe" "$probe"
awk -f "$base_transform" "$base_head_linker" > "$head_linker_x14"
awk -f "$retarget_transform" "$head_linker_x14" > "$head_linker"

run_pass() {
    target=$1
    (
        cd "$repo_root"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS \
            JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon -q \
            -PrustPortRepo="$repo_root" -PphaseTwoPage="$input" \
            -PphaseTwoProbeSource="$probe" \
            -PphaseTwoHeadLinkerSource="$head_linker" \
            -I "$init" :app:stemsHeadPhaseTwoX14Probe
    ) > "$target"
}

run_pass "$tmp_dir/warmup"
run_pass "$tmp_dir/pass1"
run_pass "$tmp_dir/pass2"
grep -E '^(stemsheadphase2bachs3q3|stemsheadphase2baseline .* system 3 |stemsheadphase2retry .* system 3 queueIndex 3 )' \
    "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep -E '^(stemsheadphase2bachs3q3|stemsheadphase2baseline .* system 3 |stemsheadphase2retry .* system 3 queueIndex 3 )' \
    "$tmp_dir/pass2" > "$tmp_dir/rows2"
if ! cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2"; then
    echo "fresh Bach system-3 queue-3 Java passes are not byte-identical" >&2
    exit 1
fi
if [ "$(wc -l < "$tmp_dir/rows1" | tr -d ' ')" -ne 7 ] || \
        ! grep -q '^stemsheadphase2bachs3q3frontier headInterId 4379 corner BR .*lastIndex 2 maxIndex 2 relations 2 .*existingStem id7385:' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2bachs3q3reusematch headInterId 4379 sourceHeadId 4399 sourceCorner BL sourceSide LEFT relationGrade 3fe79fa21b3bae82 stem id7385:' "$tmp_dir/rows1" || \
        ! grep -q '^stemsheadphase2bachs3q3result headInterId 4379 linkedStem id7385:.*applied grade3fe613e185913e1e:dxbfcad42f4a207c3c:.*edgesBefore 537 edgesAfter 538 allocatorBefore 7416 allocatorAfter 7416 terminal ReturnedHeadCLinkTransaction$' "$tmp_dir/rows1"; then
    echo "Bach system-3 queue-3 Java contract differs" >&2
    cat "$tmp_dir/rows1" >&2
    exit 1
fi

sha() { shasum -a 256 "$1" | awk '{print $1}'; }
out="$repo_root/rust/oracle/stems-head-phase-two-bach-system3-order3.txt"
{
    echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Bach system-3 phase-two queue 3.'
    echo '# schema: stems-head-phase-two-bach-system3-order3-v1'
    cat "$tmp_dir/rows1"
    printf '%s\n' \
        "stemsheadphase2bachs3q3summary schema stems-head-phase-two-bach-system3-order3-v1 page BachInvention5.jpg#1 system 3 rows 7 inputSha256 $(sha "$input") baseProbeSourceSha256 $(sha "$base_probe") headLinkerSourceSha256 $(sha "$base_head_linker") baseTransformSourceSha256 $(sha "$base_transform") retargetTransformSourceSha256 $(sha "$retarget_transform") transformedHeadLinkerSourceSha256 $(sha "$head_linker") initSourceSha256 $(sha "$init") runnerSourceSha256 $(sha "$0") emittedBodySha256 $(sha "$tmp_dir/rows1") semanticPassSha256 $(sha "$tmp_dir/rows1") freshRuns 2 freshRunsByteIdentical true nativeScope BachSystem3PhaseTwoOrder3RightReusedStemAppend javaEvidence ReturnedBeforeSystem3RetryIndex4"
} > "$out"
echo "wrote $out"
