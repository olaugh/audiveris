// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import ij.process.ByteProcessor;

import java.awt.Point;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.Glyph;
import org.audiveris.omr.glyph.GlyphFactory;
import org.audiveris.omr.run.Orientation;
import org.audiveris.omr.run.RunTable;
import org.audiveris.omr.run.RunTableFactory;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Picture;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.Staff;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.beam.SpotsBuilder;
import org.audiveris.omr.step.OmrStep;

/**
 * The rungs between NO_STAFF and the beam spots BEAMS actually recognises.
 *
 * <p>{@code SpotsBuilder.buildSheetSpots} is eight transforms deep before a
 * single beam is considered -- stem-run removal, a median, a gaussian, a header
 * erase, the morphological closing, a threshold, a run table, connected
 * components -- and Audiveris caches only the last of them. Graded end to end,
 * any one of the eight failing looks exactly like the other seven failing. So
 * each is dumped here as it comes out.
 *
 * <p>The private stages are called through reflection rather than
 * reimplemented. A probe that reimplements what it is grading grades the
 * reimplementation.
 *
 * <p>The header erase is the awkward one: it needs {@code Staff.getHeaderStop},
 * which arrives with HEADERS. The buffer is therefore emitted both ways -- with
 * the erase and without -- along with the rectangles the erase paints, so a
 * port that has no HEADERS yet can be graded on the seven stages that do not
 * need one and can see exactly what the eighth would cost.
 *
 * <pre>
 *   unset JAVA_TOOL_OPTIONS
 *   JAVA_HOME=.../jdk25/Contents/Home ./gradlew --no-daemon -q \
 *     -I rust/oracle/java/staff-impacts.init.gradle :app:spotsProbe \
 *     -PspotsTargets="data/examples/chula.png:1"
 * </pre>
 */
public class SpotsProbe
{
    private SpotsProbe ()
    {
    }

    private static String digest (ByteProcessor buffer)
    {
        long hash = 0xcbf29ce484222325L;

        for (int i = 0, n = buffer.getWidth() * buffer.getHeight(); i < n; i++) {
            hash = (hash ^ (buffer.get(i) & 0xff)) * 0x100000001b3L;
        }

        return String.format("%016x", hash);
    }

    private static String digest (RunTable table)
    {
        return digest(table.getBuffer());
    }

    /**
     * Calls a private {@link SpotsBuilder} method.
     *
     * @param builder    the builder
     * @param name       method name
     * @param signature  parameter types
     * @param parameters arguments
     * @return whatever it returned
     * @throws Exception if reflection fails
     */
    private static Object call (SpotsBuilder builder,
                                String name,
                                Class<?>[] signature,
                                Object... parameters)
        throws Exception
    {
        final Method method = SpotsBuilder.class.getDeclaredMethod(name, signature);
        method.setAccessible(true);

        return method.invoke(builder, parameters);
    }

    public static void main (String[] args)
        throws Exception
    {
        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "BEAMS");
        final Field field = Main.class.getDeclaredField("cli");
        field.setAccessible(true);
        field.set(null, cli);

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

                // BEAMS itself, so the header stops and the stem scale exist
                // and the spots are the ones the real step saw.
                stub.reachStep(OmrStep.BEAMS, false);

                final Sheet sheet = stub.getSheet();
                final Picture picture = sheet.getPicture();
                final Scale scale = sheet.getScale();
                final ByteProcessor noStaff = picture.getSource(Picture.SourceKey.NO_STAFF);

                System.out.println("sheet " + path.getFileName() + "#" + wanted + " "
                        + noStaff.getWidth() + " " + noStaff.getHeight() + " " + digest(noStaff));

                // The beam thickness SpotsBuilder picks, and what it derives.
                final Integer beam = scale.getItemValue(Scale.Item.beam);
                final Integer smallBeam = scale.getItemValue(Scale.Item.smallBeam);
                final int used = (smallBeam != null) ? Math.min(beam, smallBeam) : beam;
                System.out.println("scale beam " + beam + " smallBeam " + smallBeam + " used "
                        + used + " maxStem " + scale.getMaxStem() + " interline "
                        + scale.getInterline());

                // The erase rectangles, so a port without HEADERS can be given
                // them rather than guessing at them.
                //
                // SpotsBuilder's own constant, read reflectively. Constructing
                // a fresh Scale.Fraction here instead wedges the process: a
                // Constant declared outside a ConstantSet never finishes
                // registering, and the probe hangs after printing `scale`.
                final Field constantsField = SpotsBuilder.class.getDeclaredField("constants");
                constantsField.setAccessible(true);
                final Object spotConstants = constantsField.get(null);
                final Field marginField = spotConstants.getClass()
                        .getDeclaredField("staffVerticalMargin");
                marginField.setAccessible(true);
                System.out.println("verticalmargin "
                        + scale.toPixels((Scale.Fraction) marginField.get(spotConstants)));

                for (SystemInfo system : sheet.getSystems()) {
                    final Staff firstStaff = system.getFirstStaff();
                    final Staff lastStaff = system.getLastStaff();

                    for (Staff staff : system.getStaves()) {
                        System.out.println("header system " + system.getId() + " staff "
                                + staff.getId() + " stop "
                                + ((staff.getHeader() != null) ? staff.getHeaderStop() : "none"));
                    }

                    for (Staff staff : system.getStaves()) {
                        if (staff.getHeader() == null) {
                            continue;
                        }
                        final int stop = staff.getHeaderStop();
                        System.out.println("erase system " + system.getId() + " x "
                                + system.getBounds().x + " stop " + stop + " firstline "
                                + firstStaff.getFirstLine().yAt(stop) + " lastline "
                                + lastStaff.getLastLine().yAt(stop));

                        break;
                    }
                }

                final SpotsBuilder builder = new SpotsBuilder(sheet);

                // Stage by stage, through SpotsBuilder's own private methods.
                final RunTableFactory destemFactory = new RunTableFactory(
                        Orientation.HORIZONTAL,
                        new RunTableFactory.LengthFilter(scale.getMaxStem()));
                final ByteProcessor destemmed = destemFactory.createTable(noStaff).getBuffer();
                System.out.println("destemmed " + scale.getMaxStem() + " " + digest(destemmed));

                final ByteProcessor median = picture.medianFiltered(destemmed);
                System.out.println("median " + digest(median));

                final ByteProcessor gaussian = picture.gaussianFiltered(median);
                System.out.println("gaussian " + digest(gaussian));

                // getBuffer() runs exactly that chain; if these disagree the
                // probe is wrong rather than the port.
                final ByteProcessor whole = (ByteProcessor) call(
                        builder, "getBuffer", new Class<?>[] {});
                System.out.println("getbuffer " + digest(whole));

                final ByteProcessor unerased = (ByteProcessor) gaussian.duplicate();
                call(builder, "close", new Class<?>[] { ByteProcessor.class, double.class },
                        unerased, (double) used);
                System.out.println("closednoerase " + digest(unerased));

                final ByteProcessor erased = (ByteProcessor) gaussian.duplicate();
                call(builder, "eraseHeaderAreas", new Class<?>[] { ByteProcessor.class }, erased);
                System.out.println("erased " + digest(erased));

                final ByteProcessor closed = (ByteProcessor) erased.duplicate();
                call(builder, "close", new Class<?>[] { ByteProcessor.class, double.class },
                        closed, (double) used);
                System.out.println("closed " + digest(closed));

                // The two binarizations the closed buffer feeds: beams at 140,
                // and the head runs Audiveris persists for HEADS at 170.
                final ByteProcessor heads = (ByteProcessor) closed.duplicate();
                heads.threshold(170);
                System.out.println("headthreshold " + digest(heads));
                final RunTable headRuns = new RunTableFactory(Orientation.VERTICAL)
                        .createTable(heads);
                System.out.println("headruns " + headRuns.getSize() + " " + digest(headRuns));

                final ByteProcessor binary = (ByteProcessor) closed.duplicate();
                binary.threshold(140);
                System.out.println("beamthreshold " + digest(binary));

                final RunTable spotTable = new RunTableFactory(Orientation.VERTICAL)
                        .createTable(binary);
                System.out.println("spotruns " + spotTable.getSize() + " " + digest(spotTable));

                // The connected components BEAMS actually looks at. Sorted by
                // position so two runs diff cleanly; Java's own order is an
                // artefact of the component walk and is pinned separately.
                final List<Glyph> spots = GlyphFactory.buildGlyphs(spotTable, new Point(0, 0));
                System.out.println("spotcount " + spots.size());

                final List<Glyph> sorted = new ArrayList<>(spots);
                sorted.sort(Comparator.comparingInt((Glyph glyph) -> glyph.getBounds().y)
                        .thenComparingInt(glyph -> glyph.getBounds().x)
                        .thenComparingInt(glyph -> glyph.getBounds().width)
                        .thenComparingInt(glyph -> glyph.getBounds().height));
                for (int i = 0; i < sorted.size(); i++) {
                    final Glyph glyph = sorted.get(i);
                    System.out.println("spot " + i + " " + glyph.getBounds().x + " "
                            + glyph.getBounds().y + " " + glyph.getBounds().width + " "
                            + glyph.getBounds().height + " " + glyph.getWeight());
                }

                // Which system each spot was dispatched to, by the same key, so
                // an ordering difference in the component walk cannot hide a
                // dispatch difference.
                for (int i = 0; i < sorted.size(); i++) {
                    final Point center = sorted.get(i).getCentroid();
                    final StringBuilder line = new StringBuilder("dispatch ").append(i).append(' ')
                            .append(center.x).append(' ').append(center.y);
                    final List<SystemInfo> relevants = new ArrayList<>();
                    sheet.getSystemManager().getSystemsOf(center, relevants);
                    for (SystemInfo system : relevants) {
                        line.append(' ').append(system.getId());
                    }
                    System.out.println(line);
                }
            }
        }
    }
}
