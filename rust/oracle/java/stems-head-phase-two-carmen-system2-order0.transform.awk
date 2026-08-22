# SPDX-License-Identifier: AGPL-3.0-or-later
# Add a bounded Carmen system-2 queue-zero phase-two C-link envelope.

function emit_helpers() {
    print "    private static String rustCarmenHex (double value)"
    print "    {"
    print "        return Long.toHexString(Double.doubleToRawLongBits(value));"
    print "    }"
    print ""
    print "    private static String rustCarmenPoint (Point2D point)"
    print "    {"
    print "        return point == null ? \"-\" : rustCarmenHex(point.getX()) + \":\" + rustCarmenHex(point.getY());"
    print "    }"
    print ""
    print "    private static String rustCarmenRectangle (Rectangle box)"
    print "    {"
    print "        return box == null ? \"-\" : box.x + \":\" + box.y + \":\" + box.width + \":\" + box.height;"
    print "    }"
    print ""
    print "    private static String rustCarmenGlyph (Glyph glyph)"
    print "    {"
    print "        return glyph == null ? \"-\" : \"id\" + glyph.getId() + \":\""
    print "                + rustCarmenRectangle(glyph.getBounds()) + \":weight\" + glyph.getWeight();"
    print "    }"
    print ""
    print "    private static String rustCarmenRelation (Relation relation)"
    print "    {"
    print "        if (relation instanceof HeadStemRelation hs) {"
    print "            return \"HeadStem:grade\" + rustCarmenHex(hs.getGrade())"
    print "                    + \":dx\" + rustCarmenHex(hs.getDx())"
    print "                    + \":extension\" + rustCarmenPoint(hs.getExtensionPoint())"
    print "                    + \":headSide\" + hs.getHeadSide();"
    print "        }"
    print "        return relation.getClass().getSimpleName();"
    print "    }"
    print ""
    print "    private static String rustCarmenRelations (Map<StemLinker, Relation> relations)"
    print "    {"
    print "        final List<String> rows = new ArrayList<>();"
    print "        for (Entry<StemLinker, Relation> entry : relations.entrySet()) {"
    print "            rows.add(entry.getKey().getId() + \":\" + rustCarmenRelation(entry.getValue()));"
    print "        }"
    print "        return rows.toString().replace(\" \", \"\");"
    print "    }"
    print ""
    print "    private static String rustCarmenGlyphs (Set<Glyph> glyphs)"
    print "    {"
    print "        final List<String> rows = new ArrayList<>();"
    print "        for (Glyph glyph : glyphs) rows.add(rustCarmenGlyph(glyph));"
    print "        return rows.toString().replace(\" \", \"\");"
    print "    }"
    print ""
}

function emit_frontier() {
    print "                final boolean rustCarmenAudit = append && head.getId() == 2318;"
    print "                if (rustCarmenAudit) {"
    print "                    System.out.printf("
    print "                            \"stemsheadphase2carmens2q0frontier headInterId %d corner %s \""
    print "                                    + \"hSide %s vSide %s refPt %s yDir %d \""
    print "                                    + \"minTail %d bestTail %d yHard %s ySoft %s \""
    print "                                    + \"lastIndex %d maxIndex %d relations %d relationRows %s \""
    print "                                    + \"glyphs %d selected %s terminal %s%n\","
    print "                            head.getId(), cName(), hSide, vSide, rustCarmenPoint(refPt), yDir,"
    print "                            params.minStemTailLg, params.bestStemTailLg, rustCarmenHex(yHard),"
    print "                            rustCarmenHex(ySoft), lastIndex, sb.maxIndex(), relations.size(),"
    print "                            rustCarmenRelations(relations), glyphs.size(), rustCarmenGlyphs(glyphs),"
    print "                            lastIndex == -1 ? \"ExpandMinusOne\""
    print "                                    : glyphs.isEmpty() ? \"NoGlyphs\" : \"ReadyForReuseStem\");"
    print "                }"
}

{
    if (index($0, "//~ Inner Classes") != 0) {
        emit_helpers()
        print
        next
    }
    if (index($0, "public boolean link (int stemProfile,") != 0) {
        in_c_link = 1
    }
    print
    if (in_c_link && index($0, "final int lastIndex = expand(") != 0) {
        in_expand_call = 1
    } else if (in_c_link && in_expand_call && index($0, "glyphs);") != 0) {
        emit_frontier()
        in_expand_call = 0
    }
    if (in_c_link && index($0, "// At this point, we have successfully linked") != 0) {
        in_c_link = 0
    }
}
