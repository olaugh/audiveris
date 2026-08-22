# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the identity-free queue-202 probe from the frozen queue-183 source.
{
    line = $0
    gsub(/Order183/, "Order202", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 202", line)
    gsub(/queue 183/, "queue 202", line)
    gsub(/queue-183/, "queue-202", line)
    gsub(/q183/, "q202", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 64 || sigOrdinals.get(head) != 61", line)
    print line
}
