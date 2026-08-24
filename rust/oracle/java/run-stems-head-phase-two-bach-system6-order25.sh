#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Freeze Bach system 6 queue 25's carried reuseStem append.
set -eu
[ -n "${JAVA_HOME:-}" ] && [ -x "$JAVA_HOME/bin/java" ] || { echo "JAVA_HOME must name frozen JDK25" >&2; exit 2; }
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
tmp_dir=$(mktemp -d /private/tmp/stems-head-phase-two-bach-s6-q25.XXXXXX)
trap 'rm -rf -- "$tmp_dir"' EXIT
probe="$tmp_dir/StemsHeadPhaseTwoPageProbe.java"
head_x14="$tmp_dir/HeadLinker-x14.java"
head="$tmp_dir/HeadLinker.java"
cp "$script_dir/StemsHeadPhaseTwoPageProbe.java" "$probe"
awk -f "$script_dir/stems-head-phase-two-x14.transform.awk" "$repo_root/app/src/main/java/org/audiveris/omr/sheet/stem/HeadLinker.java" > "$head_x14"
awk -f "$script_dir/stems-head-phase-two-bach-system6-order25.transform.awk" "$head_x14" > "$head"
run_pass() {
    (
        cd "$repo_root"
        env -u JAVA_TOOL_OPTIONS -u _JAVA_OPTIONS -u JDK_JAVA_OPTIONS JAVA_HOME="$JAVA_HOME" ./gradlew --no-daemon -q \
            -PrustPortRepo="$repo_root" -PphaseTwoPage="$repo_root/data/examples/BachInvention5.jpg" \
            -PphaseTwoProbeSource="$probe" -PphaseTwoHeadLinkerSource="$head" \
            -I "$script_dir/stems-head-phase-two-x14.init.gradle" :app:stemsHeadPhaseTwoX14Probe
    ) > "$1"
}
run_pass "$tmp_dir/warmup"
run_pass "$tmp_dir/pass1"
run_pass "$tmp_dir/pass2"
grep -E '^(stemsheadphase2bachs6q25|stemsheadphase2baseline .* system 6 |stemsheadphase2retry .* system 6 queueIndex 25 )' "$tmp_dir/pass1" > "$tmp_dir/rows1"
grep -E '^(stemsheadphase2bachs6q25|stemsheadphase2baseline .* system 6 |stemsheadphase2retry .* system 6 queueIndex 25 )' "$tmp_dir/pass2" > "$tmp_dir/rows2"
cmp -s "$tmp_dir/rows1" "$tmp_dir/rows2" || { echo "fresh Java passes differ" >&2; exit 1; }
[ "$(wc -l < "$tmp_dir/rows1" | tr -d ' ')" -eq 7 ] || { cat "$tmp_dir/rows1" >&2; exit 1; }
sha() { shasum -a 256 "$1" | awk '{print $1}'; }
out="$repo_root/rust/oracle/stems-head-phase-two-bach-system6-order25.txt"
{
 echo '# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) Bach system-6 phase-two queue 25.'
 echo '# schema: stems-head-phase-two-bach-system6-order25-v1'
 cat "$tmp_dir/rows1"
 printf '%s\n' "stemsheadphase2bachs6q25summary schema stems-head-phase-two-bach-system6-order25-v1 page BachInvention5.jpg#1 system 6 rows 7 inputSha256 $(sha "$repo_root/data/examples/BachInvention5.jpg") runnerSourceSha256 $(sha "$0") retargetTransformSourceSha256 $(sha "$script_dir/stems-head-phase-two-bach-system6-order25.transform.awk") emittedBodySha256 $(sha "$tmp_dir/rows1") semanticPassSha256 $(sha "$tmp_dir/rows1") freshRuns 2 freshRunsByteIdentical true nativeScope BachSystem6PhaseTwoOrder25ReuseStemAppend javaEvidence ReturnedBeforeSystem6RetryIndex26"
} > "$out"
echo "wrote $out"
