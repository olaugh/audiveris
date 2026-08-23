# SPDX-License-Identifier: AGPL-3.0-or-later
{
    line = $0
    gsub(/Order183/, "Order208", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 208", line)
    gsub(/queue 183/, "queue 208", line)
    gsub(/queue-183/, "queue-208", line)
    gsub(/q183/, "q208", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 55 || sigOrdinals.get(head) != 67", line)
    print line
}
