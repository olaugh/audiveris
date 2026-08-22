# SPDX-License-Identifier: AGPL-3.0-or-later
# Retarget queue-18 instrumentation to queue 19 / Java Inter 1069.
{
    gsub("1061", "1069")
    gsub("cucarachas1q18", "cucarachas1q19")
    print
}
