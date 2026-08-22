# SPDX-License-Identifier: AGPL-3.0-or-later
# Retarget the authenticated Cucaracha system-1 queue-6 instrumentation to
# queue 7 / Java Inter 1095 without changing the measured envelope.
{
    gsub("1083", "1095")
    gsub("cucarachas1q6", "cucarachas1q7")
    print
}
