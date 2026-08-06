// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import ij.process.ByteProcessor;

import java.lang.reflect.Field;
import java.nio.file.Path;
import java.nio.file.Paths;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.image.MorphoProcessor;
import org.audiveris.omr.image.StructureElement;
import org.audiveris.omr.run.Orientation;
import org.audiveris.omr.run.RunTable;
import org.audiveris.omr.run.RunTableFactory;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Picture;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.step.OmrStep;

/**
 * A unit oracle for grayscale morphology: the structuring element, and the
 * closing that BEAMS starts from.
 *
 * <p>The existing beams oracle grades the closing only by its consequences,
 * because Audiveris caches the closed buffer nowhere -- it is a local in
 * {@code SpotsBuilder.buildSheetSpots}. That makes a wrong structuring element
 * and a wrong closing indistinguishable from a wrong beam recogniser. This
 * probe reaches past that by calling {@link MorphoProcessor#close} directly, so
 * each layer can be graded on its own:
 *
 * <ol>
 * <li>{@code se} / {@code semask} / {@code sevect} -- the structuring element,
 * dumped rather than hashed. It is small, and a disk that is one cell wrong is
 * far easier to see as a picture than as a digest.</li>
 * <li>{@code synth} -- closing over generated buffers, whose generator is a
 * plain LCG so the port can reproduce the input exactly without shipping
 * fixtures. Small ones are dumped in full, since a diff that points at a pixel
 * beats one that says only "different".</li>
 * <li>{@code sheet} -- closing over a real staff-free page, at the radius
 * SpotsBuilder would actually use.</li>
 * </ol>
 *
 * <p>It also emits digests of the four buffers {@code SpotsBuilder.getBuffer}
 * passes through -- stem-run removal, median, gaussian -- which morphology does
 * not need but the step around it will.
 *
 * <pre>
 *   unset JAVA_TOOL_OPTIONS
 *   JAVA_HOME=.../jdk25/Contents/Home ./gradlew --no-daemon -q \
 *     -I rust/oracle/java/staff-impacts.init.gradle :app:morphoProbe \
 *     -PmorphoTargets="data/examples/chula.png:1"
 * </pre>
 *
 * <p>Clear {@code JAVA_TOOL_OPTIONS}: a proxy banner on stdout corrupts every
 * parsed line.
 */
public class MorphoProbe
{
    /** Radii dumped in full. Chosen to straddle the parity of the mask width. */
    private static final float[] DUMPED_RADII =
    { 0.5f, 1.0f, 1.5f, 2.0f, 2.5f, 3.0f, 3.5f, 4.0f, 4.5f, 5.0f, 6.0f, 7.5f };

    private MorphoProbe ()
    {
    }

    //-----------//
    // synthetic //
    //-----------//
    /**
     * A reproducible gray buffer.
     *
     * <p>An LCG rather than a fixture file: the port has to generate the same
     * input, and a formula travels where a binary blob does not. The
     * multiply is a Java {@code int} multiply, so it wraps -- the port must
     * wrap too rather than promote.
     *
     * @param width  buffer width
     * @param height buffer height
     * @param seed   generator seed
     * @return the buffer
     */
    private static ByteProcessor synthetic (int width,
                                            int height,
                                            int seed)
    {
        final ByteProcessor buffer = new ByteProcessor(width, height);
        int state = seed;

        for (int i = 0, n = width * height; i < n; i++) {
            state = (state * 1103515245) + 12345;
            buffer.set(i, (state >>> 16) & 0xff);
        }

        return buffer;
    }

    //--------//
    // shapes //
    //--------//
    /**
     * A white buffer with black bars and gaps in it.
     *
     * <p>Noise exercises the min/max arithmetic; this exercises what closing is
     * for. The gaps are sized around the radii above so that some close and
     * some do not.
     *
     * @param width  buffer width
     * @param height buffer height
     * @return the buffer
     */
    private static ByteProcessor shapes (int width,
                                         int height)
    {
        final ByteProcessor buffer = new ByteProcessor(width, height);

        for (int i = 0, n = width * height; i < n; i++) {
            buffer.set(i, 255);
        }

        for (int y = 0; y < height; y++) {
            for (int x = 0; x < width; x++) {
                final boolean bar = ((y % 11) < 4) && ((x % 13) > (x / 37));
                final boolean blob = (((x - 20) * (x - 20)) + ((y - 12) * (y - 12))) < 30;

                if (bar || blob) {
                    buffer.set(x + (y * width), (x * 7) % 64);
                }
            }
        }

        return buffer;
    }

    //--------//
    // digest //
    //--------//
    /**
     * FNV-1a over the buffer, the same digest the other probes use.
     *
     * @param buffer the buffer to hash
     * @return the digest
     */
    private static String digest (ByteProcessor buffer)
    {
        long hash = 0xcbf29ce484222325L;

        for (int i = 0, n = buffer.getWidth() * buffer.getHeight(); i < n; i++) {
            hash = (hash ^ (buffer.get(i) & 0xff)) * 0x100000001b3L;
        }

        return String.format("%016x", hash);
    }

    //------//
    // dump //
    //------//
    /**
     * Print every pixel, one row per line, as two hex digits each.
     *
     * @param tag    record name
     * @param buffer the buffer to print
     */
    private static void dump (String tag,
                              ByteProcessor buffer)
    {
        for (int y = 0; y < buffer.getHeight(); y++) {
            final StringBuilder row = new StringBuilder(tag).append(' ').append(y).append(' ');

            for (int x = 0; x < buffer.getWidth(); x++) {
                row.append(String.format("%02x", buffer.get(x + (y * buffer.getWidth()))));
            }

            System.out.println(row);
        }
    }

    //---------------//
    // describeShape //
    //---------------//
    /**
     * Print a structuring element: its dimensions, its mask as a picture, and
     * the vector form that {@code close} actually reads.
     *
     * @param radius the radius to build at
     */
    private static void describeShape (float radius)
    {
        final int[] offset =
        { 0, 0 };
        final StructureElement se = new StructureElement(0, 1, radius, offset);
        final int[] mask = se.getMask();
        final int[][] vect = se.getVect();

        System.out.println(String.format(
                "se %.4f %d %d %d %d",
                radius,
                se.getWidth(),
                se.getHeight(),
                se.getArea(),
                vect.length));

        for (int y = 0; y < se.getHeight(); y++) {
            final StringBuilder row = new StringBuilder("semask ").append(y).append(' ');

            for (int x = 0; x < se.getWidth(); x++) {
                row.append((mask[x + (y * se.getWidth())] > 0) ? '#' : '.');
            }

            System.out.println(row);
        }

        // Order matters to nothing in close -- it is a min and a max -- but it
        // is part of the port, and a mask that is right while its vector is
        // reordered is a bug waiting for the next operator that does care.
        for (int i = 0; i < vect.length; i++) {
            System.out.println("sevect " + i + " " + vect[i][0] + " " + vect[i][1] + " "
                    + vect[i][2] + " " + vect[i][3]);
        }
    }

    //-------//
    // close //
    //-------//
    /**
     * Close a buffer with a disk of the given radius.
     *
     * @param buffer buffer, modified in place
     * @param radius disk radius
     */
    private static void close (ByteProcessor buffer,
                               float radius)
    {
        final int[] offset =
        { 0, 0 };
        new MorphoProcessor(new StructureElement(0, 1, radius, offset)).close(buffer);
    }

    public static void main (String[] args)
        throws Exception
    {
        for (float radius : DUMPED_RADII) {
            describeShape(radius);
        }

        // The generators themselves, before any closing. Without these a port
        // whose LCG has drifted fails every digest below at once and blames
        // morphology for it.
        for (int[] size : new int[][] { { 24, 16 }, { 201, 97 } }) {
            System.out.println(String.format(
                    "synthsrc noise %d %d %s",
                    size[0],
                    size[1],
                    digest(synthetic(size[0], size[1], (size[0] == 24) ? 12345 : 987654321))));
            System.out.println(String.format(
                    "synthsrc shapes %d %d %s",
                    size[0],
                    size[1],
                    digest(shapes(size[0], size[1]))));
        }

        // Closing over generated input. The 24x16 pair is dumped whole; the
        // larger ones are digested, because their value is coverage of the
        // arithmetic rather than legibility.
        for (float radius : new float[] { 1.5f, 3.5f }) {
            final ByteProcessor noise = synthetic(24, 16, 12345);
            close(noise, radius);
            System.out.println(String.format("synth noise 24 16 %.4f %s", radius, digest(noise)));
            dump(String.format("synthpix.noise.%.1f", radius), noise);

            final ByteProcessor drawn = shapes(24, 16);
            close(drawn, radius);
            System.out.println(String.format("synth shapes 24 16 %.4f %s", radius, digest(drawn)));
            dump(String.format("synthpix.shapes.%.1f", radius), drawn);
        }

        for (float radius : new float[] { 0.5f, 2.0f, 3.5f, 7.5f }) {
            final ByteProcessor noise = synthetic(201, 97, 987654321);
            close(noise, radius);
            System.out.println(String.format("synth noise 201 97 %.4f %s", radius, digest(noise)));

            final ByteProcessor drawn = shapes(201, 97);
            close(drawn, radius);
            System.out.println(String.format("synth shapes 201 97 %.4f %s", radius, digest(drawn)));
        }

        if (args.length == 0) {
            return;
        }

        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "GRID");
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

                stub.reachStep(OmrStep.GRID, false);

                final Sheet sheet = stub.getSheet();
                final Picture picture = sheet.getPicture();
                final Scale scale = sheet.getScale();
                final ByteProcessor noStaff = picture.getSource(Picture.SourceKey.NO_STAFF);

                System.out.println("sheet " + path.getFileName() + "#" + wanted + " "
                        + noStaff.getWidth() + " " + noStaff.getHeight() + " " + digest(noStaff));

                // The radius SpotsBuilder would use here, reported term by term
                // so a port that disagrees says where rather than only that.
                final Integer beam = scale.getItemValue(Scale.Item.beam);
                final Integer smallBeam = scale.getItemValue(Scale.Item.smallBeam);
                final int used = (smallBeam != null) ? Math.min(beam, smallBeam) : beam;
                final double diameter = used * 0.8;
                final float radius = (float) (diameter - 1) / 2;
                System.out.println(String.format(
                        "radius beam %s smallBeam %s used %d diameter %.6f radius %.6f",
                        beam,
                        smallBeam,
                        used,
                        diameter,
                        radius));

                // Closing on NO_STAFF rather than on what SpotsBuilder feeds
                // it: the port already reproduces NO_STAFF exactly, so this
                // grades morphology alone. The stem/median/gaussian chain below
                // is what the rest of the step will need.
                for (float r : new float[] { radius, 1.5f, 3.5f }) {
                    final ByteProcessor copy = (ByteProcessor) noStaff.duplicate();
                    close(copy, r);
                    System.out.println(String.format("closed %.6f %s", r, digest(copy)));
                }

                // The rest is what BEAMS feeds the closing, not what morphology
                // needs. It wants a stem scale, which arrives with STEM_SEEDS
                // -- SpotsBuilder runs after it -- so the step is reached here
                // rather than at GRID, and only for this part.
                stub.reachStep(OmrStep.STEM_SEEDS, false);

                final RunTableFactory factory = new RunTableFactory(
                        Orientation.HORIZONTAL,
                        new RunTableFactory.LengthFilter(scale.getMaxStem()));
                final RunTable table = factory.createTable(noStaff);
                final ByteProcessor destemmed = table.getBuffer();
                System.out.println("destemmed " + scale.getMaxStem() + " " + digest(destemmed));

                final ByteProcessor median = picture.medianFiltered(destemmed);
                System.out.println("median " + digest(median));

                final ByteProcessor gaussian = picture.gaussianFiltered(median);
                System.out.println("gaussian " + digest(gaussian));

                final ByteProcessor spots = (ByteProcessor) gaussian.duplicate();
                close(spots, radius);
                System.out.println("spots " + digest(spots));
            }
        }
    }
}
