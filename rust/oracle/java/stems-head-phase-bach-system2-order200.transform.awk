# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the identity-free queue-200 probe from the frozen queue-183 source.
{
    line = $0
    gsub(/Order183/, "Order200", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 200", line)
    gsub(/queue 183/, "queue 200", line)
    gsub(/queue-183/, "queue-200", line)
    gsub(/q183/, "q200", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 42 || sigOrdinals.get(head) != 66", line)
    print line
}
