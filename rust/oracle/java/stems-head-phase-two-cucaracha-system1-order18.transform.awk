# SPDX-License-Identifier: AGPL-3.0-or-later
# Retarget queue-16 instrumentation to queue 18 / Java Inter 1061.
{
    gsub("1097", "1061")
    gsub("cucarachas1q16", "cucarachas1q18")
    print
}
