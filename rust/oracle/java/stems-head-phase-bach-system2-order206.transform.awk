# SPDX-License-Identifier: AGPL-3.0-or-later
{
    line = $0
    gsub(/Order183/, "Order206", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 206", line)
    gsub(/queue 183/, "queue 206", line)
    gsub(/queue-183/, "queue-206", line)
    gsub(/q183/, "q206", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 118 || sigOrdinals.get(head) != 211", line)
    print line
}
