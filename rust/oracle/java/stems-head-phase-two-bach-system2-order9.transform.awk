# SPDX-License-Identifier: AGPL-3.0-or-later
# Retarget the authenticated Allegretto x14 C-link instrumentation to
# Bach system 2 queue 9 / Java Inter 3641.
function emit_after_reuse() {
    print "                    if (append && head.getId() == 3641) {"
    print "                        System.out.printf("
    print "                                \"stemsheadphase2bachs2q9reuse headInterId %d lastIndex %d selectedStem %s terminal ReturnedFromReuseStem%n\","
    print "                                head.getId(), lastIndex, rustStem(stem));"
    print "                    }"
}

function emit_reuse_match() {
    print "                            if (head.getId() == 3641) {"
    print "                                final StemInter rustReuse = (StemInter) sig.getOppositeInter(h, r);"
    print "                                System.out.printf("
    print "                                        \"stemsheadphase2bachs2q9reusematch headInterId %d sourceHeadId %d sourceCorner %s sourceSide %s relationGrade %s stem %s terminal SelectedReuseStem%n\","
    print "                                        head.getId(), h.getId(), cl.cName(),"
    print "                                        cl.getSLinker().getHorizontalSide(),"
    print "                                        rustHex(hsRel.getGrade()), rustStem(rustReuse));"
    print "                            }"
}

function emit_committed_result() {
    print "                if (append && head.getId() == 3641) {"
    print "                    System.out.printf("
    print "                            \"stemsheadphase2bachs2q9commit headInterId %d linkedStem %s relationRows %s vertices %d edges %d allocator %d terminal ReturnedHeadCLinkTransaction%n\","
    print "                            head.getId(), rustStem(stem), rustRelations(relations),"
    print "                            sig.vertexSet().size(), sig.edgeSet().size(),"
    print "                            system.getSheet().getPersistentIdGenerator().get());"
    print "                }"
}

{
    gsub("head.getId[(][)] == 1777", "head.getId() == 3641")
    gsub("stemsheadphase2x14", "stemsheadphase2bachs2q9")
    if (index($0, "private StemInter reuseStem (int lastIndex)") != 0) {
        in_reuse_stem = 1
    }
    if (index($0, "// At this point, we have successfully linked") != 0) {
        emit_committed_result()
    }
    print
    if (index($0, "stem = reuseStem(lastIndex);") != 0) {
        emit_after_reuse()
    }
    if (in_reuse_stem && index($0, "if (hsRel.getHeadSide() == cl.getSLinker().getHorizontalSide())") != 0) {
        emit_reuse_match()
    }
    if (in_reuse_stem && index($0, "return null;") != 0) {
        in_reuse_stem = 0
    }
}
