# SPDX-License-Identifier: AGPL-3.0-or-later
# Retarget queue-9 instrumentation to queue 16 / Java Inter 1097.
{
    gsub("1091", "1097")
    gsub("cucarachas1q9", "cucarachas1q16")
    print
}
