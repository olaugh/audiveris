# SPDX-License-Identifier: AGPL-3.0-or-later
# Retarget the authenticated queue-7 instrumentation to Java queue 8 /
# Inter 1166. This entry is already linked, so only the page retry row emits.
{
    gsub("1095", "1166")
    gsub("cucarachas1q7", "cucarachas1q8")
    print
}
