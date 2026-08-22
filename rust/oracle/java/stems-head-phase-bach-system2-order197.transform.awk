# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the identity-free queue-197 probe from the frozen queue-183 source.
{
    line = $0
    gsub(/Order183/, "Order197", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 197", line)
    gsub(/queue 183/, "queue 197", line)
    gsub(/queue-183/, "queue-197", line)
    gsub(/q183/, "q197", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 30 || sigOrdinals.get(head) != 95", line)
    print line
}
