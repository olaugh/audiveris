// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.lang.reflect.Field;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.List;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sig.GradeImpacts;
import org.audiveris.omr.sig.inter.BarlineInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.step.OmrStep;

/**
 * Prints the staff-vertical impacts behind every barline the GRID step promotes.
 *
 * <p>The Rust port reproduces 414 of 420 barline grades on the example corpus and
 * misses six by 0.0036 to 0.0048, every one of them the leftmost or rightmost
 * barline of its staff. A grade is a weighted geometric mean of six impacts and
 * {@code sheet#N.xml} persists only the product, so the residual cannot be
 * attributed from the existing oracles. This prints the terms.
 *
 * <p>It needs no change to the production tree, twice over:
 * {@code AbstractInter.getImpacts()} keeps the {@code GradeImpacts} the peak was
 * built with, and Audiveris already has a hook for running a class over a
 * processed book, so no step machinery has to be reproduced either.
 *
 * <p>Run it as that hook, which is what supplies the CLI singleton the step
 * engine reads:
 *
 * <pre>
 *   unset JAVA_TOOL_OPTIONS
 *   JAVA_HOME=.../jdk25/Contents/Home ./gradlew --no-daemon -q \
 *     -I rust/oracle/java/staff-impacts.init.gradle :app:staffImpactsProbe \
 *     -PimpactPages="data/examples/carmen.png data/examples/cucaracha.png"
 * </pre>
 *
 * <p>Clearing {@code JAVA_TOOL_OPTIONS} is not optional: a proxy banner on
 * stdout corrupts every parsed line, which has bitten this oracle before.
 */
public class StaffImpactsProbe
{
    private StaffImpactsProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
    {
        // The step engine reads Main.getCli() while running a step, and driving
        // a Book directly leaves it null. Audiveris's own -run hook would supply
        // it, but that hook fires after the book is stored and its sheets
        // disposed, and the probe would then reload sheet#1.xml -- which does
        // not persist impacts, so every one would read null. Setting the
        // singleton is what lets the impacts be read while they still exist.
        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "GRID");
        final Field field = Main.class.getDeclaredField("cli");
        field.setAccessible(true);
        field.set(null, cli);

        for (String arg : args) {
            final Path path = Paths.get(arg).toAbsolutePath();
            final Book book = new Book(path);
            book.createStubs();

            for (SheetStub stub : book.getValidStubs()) {
                stub.reachStep(OmrStep.GRID, false);
                final Sheet sheet = stub.getSheet();

                for (SystemInfo system : sheet.getSystems()) {
                    for (Inter inter : system.getSig().inters(BarlineInter.class)) {
                        final GradeImpacts impacts = inter.getImpacts();

                        if (impacts == null) {
                            continue;
                        }

                        final StringBuilder line = new StringBuilder("IMPACTS ");
                        line.append(path.getFileName());
                        line.append(" sheet ").append(stub.getNumber());
                        line.append(" staff ").append(
                                (inter.getStaff() != null) ? inter.getStaff().getId() : -1);
                        line.append(" x ").append(
                                String.format("%.1f", inter.getBounds().getCenterX()));
                        line.append(" grade ").append(String.format("%.9f", inter.getGrade()));

                        for (int i = 0; i < impacts.getImpactCount(); i++) {
                            line.append(' ').append(impacts.getName(i)).append(' ').append(
                                    String.format("%.9f", impacts.getImpact(i)));
                        }

                        System.out.println(line);
                    }
                }
            }
        }

        System.exit(0);
    }
}
