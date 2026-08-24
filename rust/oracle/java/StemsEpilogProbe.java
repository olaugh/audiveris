// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sig.SIGraph;
import org.audiveris.omr.sig.inter.AbstractBeamInter;
import org.audiveris.omr.sig.inter.BeamHookInter;
import org.audiveris.omr.sig.inter.BeamInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.relation.BeamHeadRelation;
import org.audiveris.omr.sig.relation.Relation;
import org.audiveris.omr.step.OmrStep;

/** Exact page epilog evidence for Java {@code StemsStep.doEpilog}. */
public final class StemsEpilogProbe {
    private StemsEpilogProbe() {}

    public static void main(String[] args) throws Exception {
        if (args.length != 1) throw new IllegalArgumentException("expected one image");
        CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "STEMS");
        var cliField = Main.class.getDeclaredField("cli");
        cliField.setAccessible(true);
        cliField.set(null, cli);
        org.audiveris.omr.ui.symbol.MusicFont.checkMusicFont();

        Path path = Path.of(args[0]).toAbsolutePath();
        Book book = new Book(path);
        book.createStubs();
        SheetStub stub = book.getValidStubs().get(0);
        stub.reachStep(OmrStep.HEADS, false);
        Sheet sheet = stub.getSheet();
        List<List<AbstractBeamInter>> before = new ArrayList<>();
        for (SystemInfo system : sheet.getSystems()) {
            List<AbstractBeamInter> beams = new ArrayList<>();
            for (Inter inter : system.getSig().inters(AbstractBeamInter.class)) {
                if (inter instanceof BeamInter || inter instanceof BeamHookInter) {
                    beams.add((AbstractBeamInter) inter);
                }
            }
            before.add(beams);
        }

        stub.reachStep(OmrStep.STEMS, false);
        String page = path.getFileName() + "#1";
        for (int index = 0; index < sheet.getSystems().size(); index++) {
            SystemInfo system = sheet.getSystems().get(index);
            SIGraph sig = system.getSig();
            List<Integer> removed = before.get(index).stream()
                    .filter(Inter::isRemoved).map(Inter::getId).sorted().toList();
            List<Long> beamHeadGrades = new ArrayList<>();
            for (Relation relation : sig.edgeSet()) {
                if (relation instanceof BeamHeadRelation beamHead) {
                    beamHeadGrades.add(Double.doubleToLongBits(beamHead.getGrade()));
                }
            }
            List<Long> contextual = new ArrayList<>();
            int nullContextual = 0;
            for (Inter inter : sig.vertexSet()) {
                if (inter.getContextualGrade() == null) nullContextual++;
                else contextual.add(Double.doubleToLongBits(inter.getContextualGrade()));
            }
            int activeOrdinal = 0;
            for (Inter inter : sig.vertexSet()) {
                System.out.printf(
                        "stemsepiloggrade system %d ordinal %d id %d class %s shape %s "
                                + "intrinsic %016x contextual %016x%n",
                        system.getId(), activeOrdinal++, inter.getId(),
                        inter.getClass().getSimpleName(), inter.getShape(),
                        Double.doubleToLongBits(inter.getGrade()),
                        Double.doubleToLongBits(inter.getContextualGrade()));
            }
            System.out.printf(
                    "stemsepilog page %s system %d removedCount %d removed %s "
                            + "beamHeadCount %d beamHeadGradeSha256 %s contextualCount %d "
                            + "contextualNull %d contextualGradeSha256 %s "
                            + "contextualGradeFnv64 %016x vertexCount %d%n",
                    page, system.getId(), removed.size(), removed, beamHeadGrades.size(),
                    digest(beamHeadGrades), contextual.size(), nullContextual,
                    digest(contextual), fnv(contextual), sig.vertexSet().size());
        }
        System.exit(0);
    }

    private static String digest(List<Long> values) throws Exception {
        Collections.sort(values);
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        for (long value : values) {
            digest.update(String.format("%016x%n", value).getBytes(StandardCharsets.UTF_8));
        }
        return java.util.HexFormat.of().formatHex(digest.digest());
    }

    private static long fnv(List<Long> values) {
        Collections.sort(values);
        long digest = 0xcbf29ce484222325L;
        for (long value : values) {
            for (int shift = 56; shift >= 0; shift -= 8) {
                digest ^= (value >>> shift) & 0xffL;
                digest *= 0x100000001b3L;
            }
        }
        return digest;
    }
}
