# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the identity-free queue-184 probe from the frozen queue-183 source.
{
    line = $0
    gsub(/Order183/, "Order184", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 184", line)
    gsub(/queue 183/, "queue 184", line)
    gsub(/queue-183/, "queue-184", line)
    gsub(/q183/, "q184", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 25 || sigOrdinals.get(head) != 93", line)
    print line
}
