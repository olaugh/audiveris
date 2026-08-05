// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.geom.Line2D;
import java.lang.reflect.Field;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.Staff;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sig.inter.BarlineInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.HorizontalSide;

/**
 * Generates {@code oracle/grid-pdf.txt}: the GRID step's output on PDF sheets.
 *
 * <p>The nine {@code data/examples} pages that {@code grid-sig.txt},
 * {@code grid-barlines.txt} and {@code grid-completed-lines.txt} pin are all
 * PNG or JPEG. The port's PDF path reproduces PDFBox's rendered raster exactly
 * on all 189 corpus pages, but nothing graded what GRID then made of one, and
 * "the pixels are right" is a weaker claim than "the recognition is right":
 * binarization sits between them and a single flipped pixel has moved a staff
 * line before.
 *
 * <p>Arguments are {@code <path>:<sheet>} with the sheet counted from one, as
 * {@code Loader.getImage(int id)} counts. Records mirror {@code grid-sig.txt}'s
 * shape so the comparison is the one already proven on the example corpus,
 * except that grades are printed at full precision rather than the three
 * decimals {@code sheet#N.xml} persists -- this reads the live SIG, so there is
 * no reason to throw the precision away.
 *
 * <pre>
 *   unset JAVA_TOOL_OPTIONS
 *   JAVA_HOME=.../jdk25/Contents/Home ./gradlew --no-daemon -q \
 *     -I rust/oracle/java/staff-impacts.init.gradle :app:gridPdfProbe \
 *     -PgridPdfPages="/path/IMSLP00709-Schumann_.pdf:2"
 * </pre>
 */
public class GridPdfProbe
{
    private GridPdfProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
    {
        // The step engine reads Main.getCli() mid-step; driving a Book directly
        // leaves it null. See StaffImpactsProbe for why the -run hook is not
        // usable here either.
        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "GRID");
        final Field field = Main.class.getDeclaredField("cli");
        field.setAccessible(true);
        field.set(null, cli);

        for (String arg : args) {
            final int colon = arg.lastIndexOf(':');
            final Path path = Paths.get(arg.substring(0, colon)).toAbsolutePath();
            final int wanted = Integer.parseInt(arg.substring(colon + 1));

            final Book book = new Book(path);
            book.createStubs();

            for (SheetStub stub : book.getValidStubs()) {
                if (stub.getNumber() != wanted) {
                    continue;
                }

                System.out.println("page " + path.getFileName() + "#" + wanted);

                // A sheet with no staves -- a cover, a title page -- raises
                // rather than returning empty, and that outcome is itself a
                // parity fact: the port has to refuse the same sheets.
                try {
                    stub.reachStep(OmrStep.GRID, false);
                } catch (Exception ex) {
                    Throwable cause = ex;
                    while (cause.getCause() != null) {
                        cause = cause.getCause();
                    }
                    System.out.println("failed " + cause.getMessage());
                    continue;
                }

                final Sheet sheet = stub.getSheet();

                final List<Staff> staves = new ArrayList<>();
                for (SystemInfo system : sheet.getSystems()) {
                    staves.addAll(system.getStaves());
                }
                for (Staff staff : staves) {
                    System.out.println("staff " + staff.getId() + " "
                            + staff.getAbscissa(HorizontalSide.LEFT) + " "
                            + staff.getAbscissa(HorizontalSide.RIGHT) + " "
                            + staff.getLines().size());
                }

                for (SystemInfo system : sheet.getSystems()) {
                    for (Inter inter : system.getSig().inters(BarlineInter.class)) {
                        final BarlineInter bar = (BarlineInter) inter;
                        final Line2D median = bar.getMedian();
                        final String staffEnd = bar.isStaffEnd(HorizontalSide.LEFT) ? "LEFT"
                                : (bar.isStaffEnd(HorizontalSide.RIGHT) ? "RIGHT" : "NONE");
                        System.out.println(String.format(
                                "barline %d %s %s %.9f %.9f %b %s %.4f %.4f %.4f %.4f",
                                (bar.getStaff() != null) ? bar.getStaff().getId() : -1,
                                bar.getShape(),
                                (bar.getWidth() != null) ? String.format("%.4f", bar.getWidth())
                                        : "null",
                                bar.getGrade(),
                                bar.getContextualGrade(),
                                bar.isFrozen(),
                                staffEnd,
                                median.getX1(),
                                median.getY1(),
                                median.getX2(),
                                median.getY2()));
                    }
                }
            }
        }

        System.exit(0);
    }
}
