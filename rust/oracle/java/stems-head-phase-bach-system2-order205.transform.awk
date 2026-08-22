# SPDX-License-Identifier: AGPL-3.0-or-later
# Derive the queue-205 existing-stem C-link probe from the frozen multi-beam source.
{
    line = $0
    gsub(/StemsHeadMultiBeamCLinkPageProbe/, "StemsHeadBachSystem2Order205CLinkProbe", line)
    gsub(/TARGET_ORDER = 182/, "TARGET_ORDER = 205", line)
    gsub(/queue-182/, "queue-205", line)
    gsub(/xOrdinals.get\(head\) != 138 \|\| sigOrdinals.get\(head\) != 149/,
            "xOrdinals.get(head) != 24 || sigOrdinals.get(head) != 210", line)
    gsub(/get\(HorizontalSide.LEFT\)/, "get(HorizontalSide.RIGHT)", line)
    gsub(/h:138:LEFT:BOTTOM/, "h:24:RIGHT:BOTTOM", line)
    gsub(/glyphRows.add\("id" \+ glyph.getId\(\) \+ ":" \+ glyphToken\(glyph\)\);/,
            "glyphRows.add((glyphToken(glyph).equals(glyphToken(candidate)) ? \"candidateGlyph\" : \"supportGlyph\") + \":\" + glyphToken(glyph));", line)
    gsub(/stemsheadmultibeam/, "stemsheadbachs2q205", line)
    print line
}
