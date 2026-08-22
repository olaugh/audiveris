# SPDX-License-Identifier: AGPL-3.0-or-later
# Retarget queue-8 instrumentation to queue 9 / Java Inter 1091. This head
# restores the identity sequence consumed one index later by Rust.
{
    gsub("1166", "1091")
    gsub("cucarachas1q8", "cucarachas1q9")
    print
}
