# SPDX-License-Identifier: AGPL-3.0-or-later
{
    line = $0
    gsub(/Order183/, "Order210", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 210", line)
    gsub(/queue 183/, "queue 210", line)
    gsub(/queue-183/, "queue-210", line)
    gsub(/q183/, "q210", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 48 || sigOrdinals.get(head) != 38", line)
    print line
}
