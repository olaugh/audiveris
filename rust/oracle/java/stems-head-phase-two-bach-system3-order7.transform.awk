# SPDX-License-Identifier: AGPL-3.0-or-later
# Retarget the authenticated x14 C-link instrumentation to Bach system 3 queue 7.
function emit_after_reuse() { print "                    if (append && head.getId() == 4140) {"; print "                        System.out.printf(\"stemsheadphase2bachs3q7reuse headInterId %d lastIndex %d selectedStem %s terminal ReturnedFromReuseStem%n\", head.getId(), lastIndex, rustStem(stem));"; print "                    }" }
function emit_reuse_match() { print "                            if (head.getId() == 4140) {"; print "                                final StemInter rustReuse = (StemInter) sig.getOppositeInter(h, r);"; print "                                System.out.printf(\"stemsheadphase2bachs3q7reusematch headInterId %d sourceHeadId %d sourceCorner %s sourceSide %s relationGrade %s stem %s terminal SelectedReuseStem%n\", head.getId(), h.getId(), cl.cName(), cl.getSLinker().getHorizontalSide(), rustHex(hsRel.getGrade()), rustStem(rustReuse));"; print "                            }" }
function emit_committed_result() { print "                if (append && head.getId() == 4140) {"; print "                    System.out.printf(\"stemsheadphase2bachs3q7commit headInterId %d linkedStem %s relationRows %s vertices %d edges %d allocator %d terminal ReturnedHeadCLinkTransaction%n\", head.getId(), rustStem(stem), rustRelations(relations), sig.vertexSet().size(), sig.edgeSet().size(), system.getSheet().getPersistentIdGenerator().get());"; print "                }" }
{
    gsub("head.getId[(][)] == 1777", "head.getId() == 4140")
    gsub("stemsheadphase2x14", "stemsheadphase2bachs3q7")
    if (index($0, "private StemInter reuseStem (int lastIndex)") != 0) in_reuse_stem = 1
    if (index($0, "// At this point, we have successfully linked") != 0) emit_committed_result()
    print
    if (index($0, "stem = reuseStem(lastIndex);") != 0) emit_after_reuse()
    if (in_reuse_stem && index($0, "if (hsRel.getHeadSide() == cl.getSLinker().getHorizontalSide())") != 0) emit_reuse_match()
    if (in_reuse_stem && index($0, "return null;") != 0) in_reuse_stem = 0
}
