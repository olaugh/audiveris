# SPDX-License-Identifier: AGPL-3.0-or-later
{
    line = $0
    gsub(/Order183/, "Order212", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 212", line)
    gsub(/queue 183/, "queue 212", line)
    gsub(/queue-183/, "queue-212", line)
    gsub(/q183/, "q212", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 116 || sigOrdinals.get(head) != 202", line)
    print line
}
