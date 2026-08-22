# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the identity-free queue-189 probe from the frozen queue-183 source.
{
    line = $0
    gsub(/Order183/, "Order189", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 189", line)
    gsub(/queue 183/, "queue 189", line)
    gsub(/queue-183/, "queue-189", line)
    gsub(/q183/, "q189", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 164 || sigOrdinals.get(head) != 51", line)
    print line
}
