# SPDX-License-Identifier: AGPL-3.0-or-later
{
    line = $0
    gsub(/Order183/, "Order211", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 211", line)
    gsub(/queue 183/, "queue 211", line)
    gsub(/queue-183/, "queue-211", line)
    gsub(/q183/, "q211", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 214 || sigOrdinals.get(head) != 87", line)
    print line
}
