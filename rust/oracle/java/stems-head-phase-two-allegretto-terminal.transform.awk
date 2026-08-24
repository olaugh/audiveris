# SPDX-License-Identifier: AGPL-3.0-or-later
# Retarget authenticated phase-two instrumentation to Allegretto's three
# carried reuseStem appends that complete systems 1 and 2.
function target() { return "(head.getId() == 1351 || head.getId() == 1347 || head.getId() == 1523)" }
function emit_after_reuse() { print "                    if (append && " target() ") {"; print "                        System.out.printf(\"stemsheadphase2allegrettoterminalreuse headInterId %d lastIndex %d selectedStem %s terminal ReturnedFromReuseStem%n\", head.getId(), lastIndex, rustStem(stem));"; print "                    }" }
function emit_reuse_match() { print "                            if (" target() ") {"; print "                                final StemInter rustReuse = (StemInter) sig.getOppositeInter(h, r);"; print "                                System.out.printf(\"stemsheadphase2allegrettoterminalreusematch headInterId %d sourceHeadId %d sourceCorner %s sourceSide %s relationGrade %s stem %s terminal SelectedReuseStem%n\", head.getId(), h.getId(), cl.cName(), cl.getSLinker().getHorizontalSide(), rustHex(hsRel.getGrade()), rustStem(rustReuse));"; print "                            }" }
function emit_committed_result() { print "                if (append && " target() ") {"; print "                    System.out.printf(\"stemsheadphase2allegrettoterminalcommit headInterId %d linkedStem %s relationRows %s vertices %d edges %d allocator %d terminal ReturnedHeadCLinkTransaction%n\", head.getId(), rustStem(stem), rustRelations(relations), sig.vertexSet().size(), sig.edgeSet().size(), system.getSheet().getPersistentIdGenerator().get());"; print "                }" }
{
    gsub("head.getId[(][)] == 1777", target())
    gsub("hSide == HorizontalSide.RIGHT && vSide == VerticalSide.BOTTOM", "true")
    gsub("stemsheadphase2x14", "stemsheadphase2allegrettoterminal")
    if (index($0, "private StemInter reuseStem (int lastIndex)") != 0) in_reuse_stem = 1
    if (index($0, "// At this point, we have successfully linked") != 0) emit_committed_result()
    print
    if (index($0, "stem = reuseStem(lastIndex);") != 0) emit_after_reuse()
    if (in_reuse_stem && index($0, "if (hsRel.getHeadSide() == cl.getSLinker().getHorizontalSide())") != 0) emit_reuse_match()
    if (in_reuse_stem && index($0, "return null;") != 0) in_reuse_stem = 0
}
