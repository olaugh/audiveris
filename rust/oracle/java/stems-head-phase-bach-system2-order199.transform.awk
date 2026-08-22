# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the identity-free queue-199 probe from the frozen queue-183 source.
{
    line = $0
    gsub(/Order183/, "Order199", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 199", line)
    gsub(/queue 183/, "queue 199", line)
    gsub(/queue-183/, "queue-199", line)
    gsub(/q183/, "q199", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 32 || sigOrdinals.get(head) != 94", line)
    print line
}
