# SPDX-License-Identifier: AGPL-3.0-or-later
{
    line = $0
    gsub(/Order183/, "Order207", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 207", line)
    gsub(/queue 183/, "queue 207", line)
    gsub(/queue-183/, "queue-207", line)
    gsub(/q183/, "q207", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 156 || sigOrdinals.get(head) != 159", line)
    print line
}
