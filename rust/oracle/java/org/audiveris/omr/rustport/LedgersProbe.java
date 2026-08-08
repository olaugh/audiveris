// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.geom.Line2D;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.ledger.LedgersBuilder;
import org.audiveris.omr.sheet.ledger.LedgersPostAnalysis;
import org.audiveris.omr.sheet.ledger.LedgersStep;
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
                stub.reachStep(OmrStep.BEAMS, false);
                final Sheet sheet = stub.getSheet();
                final LedgersStep step = new LedgersStep();
                final Method prolog = LedgersStep.class.getDeclaredMethod("doProlog", Sheet.class);
                prolog.setAccessible(true);
                final LedgersStep.Context context = (LedgersStep.Context) prolog.invoke(step, sheet);
                for (SystemInfo system : sheet.getSystems()) {
                    step.doSystem(system, context);
                }
                final Field candidates = LedgersBuilder.class.getDeclaredField("ledgerCandidates");
                candidates.setAccessible(true);
                int candidateCount = 0;
                int builderCount = 0;
                for (SystemInfo system : sheet.getSystems()) {
                    candidateCount += ((List<?>) candidates.get(context.builders.get(system))).size();
                    for (Inter inter : system.getSig().vertexSet()) {
                        if (inter instanceof LedgerInter) {
                            builderCount++;
                        }
                    }
                }
                new LedgersPostAnalysis(sheet, context).process();
                final List<String> records = new ArrayList<>();
                for (SystemInfo system : sheet.getSystems()) {
                    final List<LedgerInter> ledgers = new ArrayList<>();
                    for (Inter inter : system.getSig().vertexSet()) {
                        if (inter instanceof LedgerInter ledger) {
                            ledgers.add(ledger);
                        }
                    }
                    ledgers.sort(Comparator.comparingInt(LedgerInter::getId));
                    for (LedgerInter ledger : ledgers) {
                        final String record = format(system.getId(), ledger);
                        records.add(record);
                        System.out.println(record);
                    }
                }
                records.sort(String::compareTo);
                long hash = 0xcbf29ce484222325L;
                for (String record : records) {
                    for (byte value : (record + "\n").getBytes(java.nio.charset.StandardCharsets.UTF_8)) {
                        hash ^= Byte.toUnsignedLong(value);
                        hash *= 0x100000001b3L;
                    }
                }
                System.out.printf(
                        Locale.ROOT,
                        "ledgerbuildsummary %s#%d %d %d%n",
                        path.getFileName(),
                        wanted,
                        candidateCount,
                        builderCount);
                System.out.printf(
                        Locale.ROOT,
                        "ledgersummary %s#%d %d %016x%n",
                        path.getFileName(),
                        wanted,
                        records.size(),
                        hash);
            }
        }
        System.exit(0);
    }

    private static String format (int system,
                                  LedgerInter ledger)
    {
        final Line2D median = ledger.getMedian();
        final StringBuilder line = new StringBuilder("ledger ");
        line.append(system).append(' ');
        line.append(ledger.getStaff().getId()).append(' ');
        line.append(ledger.getIndex());
        line.append(String.format(Locale.ROOT,
                " %.9f %.9f %.9f %.9f %.9f %.9f",
                median.getX1(), median.getY1(), median.getX2(), median.getY2(),
                ledger.getThickness(), ledger.getGrade()));
        final GradeImpacts impacts = ledger.getImpacts();
        for (int i = 0; i < impacts.getImpactCount(); i++) {
            line.append(String.format(Locale.ROOT, " %.9f", impacts.getImpact(i)));
        }
        return line.toString();
    }
}
