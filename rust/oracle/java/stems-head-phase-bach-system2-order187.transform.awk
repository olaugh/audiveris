# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the queue-187 existing-stem C-link probe from the frozen multi-beam source.
{
    line = $0
    gsub(/StemsHeadMultiBeamCLinkPageProbe/, "StemsHeadBachSystem2Order187CLinkProbe", line)
    gsub(/TARGET_ORDER = 182/, "TARGET_ORDER = 187", line)
    gsub(/queue-182/, "queue-187", line)
    gsub(/xOrdinals.get\(head\) != 138 \|\| sigOrdinals.get\(head\) != 149/,
            "xOrdinals.get(head) != 178 || sigOrdinals.get(head) != 52", line)
    gsub(/h:138:LEFT:BOTTOM/, "h:178:LEFT:BOTTOM", line)
    gsub(/stemsheadmultibeam/, "stemsheadbachs2q187", line)
    print line
}
