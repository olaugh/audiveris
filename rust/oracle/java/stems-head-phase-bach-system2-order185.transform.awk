# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the identity-free queue-185 probe from the frozen queue-183 source.
{
    line = $0
    gsub(/Order183/, "Order185", line)
    gsub(/TARGET_ORDER = 183/, "TARGET_ORDER = 185", line)
    gsub(/queue 183/, "queue 185", line)
    gsub(/queue-183/, "queue-185", line)
    gsub(/q183/, "q185", line)
    gsub(/xOrdinals.get\(head\) != 62 \|\| sigOrdinals.get\(head\) != 99/,
            "xOrdinals.get(head) != 192 || sigOrdinals.get(head) != 76", line)
    print line
}
