# SPDX-License-Identifier: AGPL-3.0-or-later
# Retarget the authenticated Cucaracha system-1 queue-21 instrumentation to
# Cucaracha system 2 queue 8 / Java Inter 1388.
{
    gsub("1077", "1388")
    gsub("cucarachas1q21", "cucarachas2q8")
    print
}
