// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import ij.process.ByteProcessor;

import java.awt.Rectangle;
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
import org.audiveris.omr.sheet.Picture;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sig.GradeImpacts;
import org.audiveris.omr.sig.SIGraph;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.relation.Relation;
import org.audiveris.omr.step.OmrStep;

/**
 * The general stage oracle: every inter and relation a step leaves in the SIG.
 *
 * <p>Each stage ported so far got a bespoke probe, which is fine once and a tax
 * every time after. This is deliberately shape-agnostic: it walks the SIG as a
 * graph and prints whatever is in it, so a stage that does not exist in the port
 * yet already has a parity gate waiting, and several people can port different
 * stages without each first building a way to check their work.
 *
 * <p>What it prints per inter is the vocabulary every Audiveris recognition
 * result shares -- identity, shape, staff, bounds, intrinsic and contextual
 * grade -- plus the impacts, which are the part that says *why* a grade came
 * out where it did. Impact names come from the inter's own
 * {@link GradeImpacts}, so a head's terms and a barline's terms both print
 * without this knowing either.
 *
 * <p>Arguments are {@code <path>:<sheet>:<STEP>}, the sheet counted from one and
 * the step named as {@link OmrStep}. Output is sorted by inter id so two runs
 * diff cleanly.
 *
 * <pre>
 *   unset JAVA_TOOL_OPTIONS
 *   JAVA_HOME=.../jdk25/Contents/Home ./gradlew --no-daemon -q \
 *     -I rust/oracle/java/staff-impacts.init.gradle :app:sigProbe \
 *     -PsigTargets="data/examples/chula.png:1:GRID"
 * </pre>
 *
 * <p>Clear {@code JAVA_TOOL_OPTIONS}: a proxy banner on stdout corrupts every
 * parsed line.
 */
public class SigProbe
{
    private SigProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
    {
        // The step engine reads Main.getCli() mid-step; driving a Book directly
        // leaves it null. See StaffImpactsProbe for why the -run hook cannot be
        // used instead.
        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "GRID");
        final Field field = Main.class.getDeclaredField("cli");
        field.setAccessible(true);
        field.set(null, cli);

        for (String arg : args) {
            final String[] parts = arg.split(":");
            final Path path = Paths.get(parts[0]).toAbsolutePath();
            final int wanted = Integer.parseInt(parts[1]);
            final OmrStep step = OmrStep.valueOf(parts[2]);

            final Book book = new Book(path);
            book.createStubs();

            for (SheetStub stub : book.getValidStubs()) {
                if (stub.getNumber() != wanted) {
                    continue;
                }

                System.out.println("sheet " + path.getFileName() + "#" + wanted + " " + step);

                try {
                    stub.reachStep(step, false);
                } catch (Exception ex) {
                    Throwable cause = ex;
                    while (cause.getCause() != null) {
                        cause = cause.getCause();
                    }
                    // A refusal is a result, not an absence of one.
                    System.out.println("failed " + cause.getMessage());
                    continue;
                }

                final Sheet sheet = stub.getSheet();

                // The staff-free image, which everything downstream of GRID
                // that reads ink rather than geometry starts from. Hashed
                // rather than dumped: it is a full-page buffer, and the only
                // question worth asking of it is whether it is identical.
                final ByteProcessor noStaff = sheet.getPicture().getSource(
                        Picture.SourceKey.NO_STAFF);
                if (noStaff != null) {
                    long hash = 0xcbf29ce484222325L;
                    for (int i = 0, n = noStaff.getWidth() * noStaff.getHeight(); i < n; i++) {
                        hash = (hash ^ (noStaff.get(i) & 0xff)) * 0x100000001b3L;
                    }
                    System.out.println(String.format("nostaff %d %d %016x",
                            noStaff.getWidth(), noStaff.getHeight(), hash));
                }

                for (SystemInfo system : sheet.getSystems()) {
                    final SIGraph sig = system.getSig();

                    final List<Inter> inters = new ArrayList<>(sig.vertexSet());
                    inters.sort(Comparator.comparingInt(Inter::getId));

                    for (Inter inter : inters) {
                        System.out.println(inter(system.getId(), inter));
                    }

                    final List<Relation> relations = new ArrayList<>(sig.edgeSet());
                    relations.sort(
                            Comparator.comparingInt((Relation r) -> sig.getEdgeSource(r).getId())
                                    .thenComparingInt(r -> sig.getEdgeTarget(r).getId())
                                    .thenComparing(r -> r.getClass().getSimpleName()));

                    for (Relation relation : relations) {
                        System.out.println("relation " + system.getId() + " "
                                + sig.getEdgeSource(relation).getId() + " "
                                + sig.getEdgeTarget(relation).getId() + " "
                                + relation.getClass().getSimpleName());
                    }
                }
            }
        }

        System.exit(0);
    }

    private static String inter (int system,
                                 Inter inter)
    {
        final StringBuilder line = new StringBuilder("inter ");
        line.append(system).append(' ').append(inter.getId());
        line.append(' ').append(inter.getClass().getSimpleName());
        line.append(' ').append((inter.getShape() != null) ? inter.getShape() : "none");
        line.append(' ').append((inter.getStaff() != null) ? inter.getStaff().getId() : -1);

        final Rectangle bounds = inter.getBounds();
        if (bounds != null) {
            line.append(String.format(
                    " bounds %d %d %d %d", bounds.x, bounds.y, bounds.width, bounds.height));
        } else {
            line.append(" bounds none");
        }

        line.append(String.format(" grade %.9f", inter.getGrade()));
        final Double contextual = inter.getContextualGrade();
        line.append(" ctx ").append(
                (contextual != null) ? String.format("%.9f", contextual) : "none");
        line.append(" frozen ").append(inter.isFrozen());

        final GradeImpacts impacts = inter.getImpacts();
        if (impacts != null) {
            line.append(" impacts");
            for (int i = 0; i < impacts.getImpactCount(); i++) {
                line.append(' ').append(impacts.getName(i)).append(' ').append(
                        String.format("%.9f", impacts.getImpact(i)));
            }
        }

        return line.toString();
    }
}
