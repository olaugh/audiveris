# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the queue-201 existing-stem C-link probe from the frozen multi-beam source.
{
    line = $0
    gsub(/StemsHeadMultiBeamCLinkPageProbe/, "StemsHeadBachSystem2Order201CLinkProbe", line)
    gsub(/TARGET_ORDER = 182/, "TARGET_ORDER = 201", line)
    gsub(/queue-182/, "queue-201", line)
    gsub(/xOrdinals.get\(head\) != 138 \|\| sigOrdinals.get\(head\) != 149/,
            "xOrdinals.get(head) != 168 || sigOrdinals.get(head) != 171", line)
    gsub(/h:138:LEFT:BOTTOM/, "h:168:LEFT:TOP", line)
    gsub(/VerticalSide.BOTTOM/, "VerticalSide.TOP", line)
    gsub(/glyphRows.add\("id" \+ glyph.getId\(\) \+ ":" \+ glyphToken\(glyph\)\);/,
            "glyphRows.add((glyphToken(glyph).equals(glyphToken(candidate)) ? \"candidateGlyph\" : \"supportGlyph\") + \":\" + glyphToken(glyph));", line)
    gsub(/stemsheadmultibeam/, "stemsheadbachs2q201", line)
    print line
}
