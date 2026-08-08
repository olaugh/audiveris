// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.geom.Line2D;
import java.lang.reflect.Field;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sig.GradeImpacts;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.LedgerInter;
import org.audiveris.omr.step.OmrStep;

/** Compact final-LEDGERS oracle without the general SIG probe's unrelated records. */
public class LedgersProbe
{
    private LedgersProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
    {
        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "GRID");
        final Field field = Main.class.getDeclaredField("cli");
        field.setAccessible(true);
        field.set(null, cli);
        org.audiveris.omr.ui.symbol.MusicFont.checkMusicFont();

        for (String arg : args) {
            final String[] parts = arg.split(":");
            final Path path = Paths.get(parts[0]).toAbsolutePath();
            final int wanted = Integer.parseInt(parts[1]);
            final Book book = new Book(path);
            book.createStubs();

            for (SheetStub stub : book.getValidStubs()) {
                if (stub.getNumber() != wanted) {
                    continue;
                }
                System.out.println("sheet " + path.getFileName() + "#" + wanted);
                stub.reachStep(OmrStep.LEDGERS, false);
                final Sheet sheet = stub.getSheet();
                for (SystemInfo system : sheet.getSystems()) {
                    final List<LedgerInter> ledgers = new ArrayList<>();
                    for (Inter inter : system.getSig().vertexSet()) {
                        if (inter instanceof LedgerInter ledger) {
                            ledgers.add(ledger);
                        }
                    }
                    ledgers.sort(Comparator.comparingInt(LedgerInter::getId));
                    for (LedgerInter ledger : ledgers) {
                        emit(system.getId(), ledger);
                    }
                }
            }
        }
        System.exit(0);
    }

    private static void emit (int system,
                              LedgerInter ledger)
    {
        final Line2D median = ledger.getMedian();
        final StringBuilder line = new StringBuilder("ledger ");
        line.append(system).append(' ');
        line.append(ledger.getStaff().getId()).append(' ');
        line.append(ledger.getIndex());
        line.append(String.format(
                " %.9f %.9f %.9f %.9f %.9f %.9f",
                median.getX1(), median.getY1(), median.getX2(), median.getY2(),
                ledger.getThickness(), ledger.getGrade()));
        final GradeImpacts impacts = ledger.getImpacts();
        for (int i = 0; i < impacts.getImpactCount(); i++) {
            line.append(String.format(" %.9f", impacts.getImpact(i)));
        }
        System.out.println(line);
    }
}
