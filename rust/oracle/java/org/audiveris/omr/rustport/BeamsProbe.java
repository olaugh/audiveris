// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.Rectangle;
import java.awt.geom.Line2D;
import java.lang.reflect.Constructor;
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
import org.audiveris.omr.lag.Lag;
import org.audiveris.omr.lag.Lags;
import org.audiveris.omr.math.LineUtil;
import org.audiveris.omr.run.Orientation;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.beam.BeamStructure;
import org.audiveris.omr.step.OmrStep;

/**
 * What Java makes of each beam spot, one spot at a time.
 *
 * <p>{@code oracle/beams-chula.txt} pins the SIG at the end of BEAMS, which is
 * three stages downstream of recognition: extension, hooks and grouping all add
 * and remove beams after the fact. So a port whose per-spot recognition is
 * perfect and whose extension is missing looks, in that oracle, like a port that
 * cannot recognise beams. This one stops at {@code createBeamInters} and reports
 * per spot.
 *
 * <p>Every rejection is a record too. Java's {@code checkBeamGlyph} refuses a
 * spot for one of six named reasons before any structure exists, and a port that
 * accepts a spot Java refused is wrong in a way that no accepted-beam comparison
 * would show.
 *
 * <p>The scaled constants are dumped as well. The beam kernel already in the
 * port takes them as inputs and has only ever been given test values; a kernel
 * that is exactly right and slightly mis-parameterised produces beams that are
 * plausible and not Java's.
 *
 * <pre>
 *   unset JAVA_TOOL_OPTIONS
 *   JAVA_HOME=.../jdk25/Contents/Home ./gradlew --no-daemon -q \
 *     -I rust/oracle/java/staff-impacts.init.gradle :app:beamsProbe \
 *     -PbeamsTargets="data/examples/chula.png:1"
 * </pre>
 */
public class BeamsProbe
{
    private BeamsProbe ()
    {
    }

    private static Object field (Object target,
                                 String name)
        throws Exception
    {
        Class<?> type = target.getClass();
        while (type != null) {
            try {
                final Field found = type.getDeclaredField(name);
                found.setAccessible(true);

                return found.get(target);
            } catch (NoSuchFieldException ignored) {
                type = type.getSuperclass();
            }
        }

        throw new NoSuchFieldException(name);
    }

    private static Object call (Object target,
                                String name,
                                Class<?>[] signature,
                                Object... parameters)
        throws Exception
    {
        final Method method = target.getClass().getDeclaredMethod(name, signature);
        method.setAccessible(true);

        return method.invoke(target, parameters);
    }

    private static String line (Line2D median)
    {
        return String.format(
                "%.9f %.9f %.9f %.9f",
                median.getX1(),
                median.getY1(),
                median.getX2(),
                median.getY2());
    }

    public static void main (String[] args)
        throws Exception
    {
        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "BEAMS");
        final Field cliField = Main.class.getDeclaredField("cli");
        cliField.setAccessible(true);
        cliField.set(null, cli);

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

                // STEM_SEEDS, not BEAMS. Headers and the stem scale are both
                // in place by then, and the spots still exist: BEAMS's own
                // epilog removes every BEAM_SPOT glyph, so a probe that lets
                // the step finish finds nothing to report on.
                stub.reachStep(OmrStep.STEM_SEEDS, false);

                final Sheet sheet = stub.getSheet();
                final Scale scale = sheet.getScale();

                // BeamsStep's prolog, run here instead: it builds the spots,
                // fills the lag, and dispatches to systems.
                final Lag spotLag = new org.audiveris.omr.lag.BasicLag(
                        Lags.SPOT_LAG,
                        org.audiveris.omr.sheet.beam.SpotsBuilder.SPOT_ORIENTATION);
                sheet.getLagManager().setLag(Lags.SPOT_LAG, spotLag);
                new org.audiveris.omr.sheet.beam.SpotsBuilder(sheet).buildSheetSpots(spotLag);
                final Integer beamHeight = scale.getItemValue(Scale.Item.beam);

                System.out.println("sheet " + path.getFileName() + "#" + wanted + " interline "
                        + scale.getInterline() + " beam " + beamHeight + " smallBeam "
                        + scale.getItemValue(Scale.Item.smallBeam));

                // The scaled constants, so the port's own derivation answers to
                // Java's rather than to a plausible reading of the source.
                final Class<?> itemParamsClass = Class.forName(
                        "org.audiveris.omr.sheet.beam.BeamsBuilder$ItemParameters");
                final Constructor<?> ctor = itemParamsClass.getDeclaredConstructor(
                        Scale.class, double.class, boolean.class);
                ctor.setAccessible(true);
                final Object itemParams = ctor.newInstance(scale, (double) beamHeight, false);
                for (String name : new String[] { "minBeamWidthLow", "minBeamWidthHigh",
                        "minHookWidthLow", "minHookWidthHigh", "maxHookWidth", "minHeightLow",
                        "typicalHeight", "maxHeightHigh", "cornerMargin", "maxItemXGap",
                        "coreSectionWidth" }) {
                    System.out.println("itemparam " + name + " " + field(itemParams, name));
                }

                final Class<?> builderClass = Class.forName(
                        "org.audiveris.omr.sheet.beam.BeamsBuilder");
                final Field builderConstants = builderClass.getDeclaredField("constants");
                builderConstants.setAccessible(true);
                final Object constants = builderConstants.get(null);
                for (String name : new String[] { "maxSideBeamDx", "minBeamsGapX", "maxBeamsGapX",
                        "maxBeamsGapY", "beamsXMargin", "maxStemBeamGapX", "maxStemBeamGapY",
                        "maxExtensionToStem", "maxExtensionToSpot", "beltMarginDx",
                        "beltMarginDy", "cueXMargin", "cueYMargin", "cueBoxDx", "cueBoxDy",
                        "cueMinBeamHeadDy" }) {
                    System.out.println("sheetparam " + name + " "
                            + scale.toPixels((Scale.Fraction) field(constants, name)));
                }
                System.out.println("sheetparam maxDistanceToBorder " + scale.toPixelsDouble(
                        (Scale.Fraction) field(constants, "maxDistanceToBorder")));

                // Every spot, in Java's own per-system order, with either the
                // reason it was refused or the structure it produced.
                int index = 0;
                for (SystemInfo system : sheet.getSystems()) {
                    final List<Glyph> spots = new ArrayList<>(system.getGroupedGlyphs(
                            org.audiveris.omr.glyph.GlyphGroup.BEAM_SPOT));
                    // BeamsBuilder's own order: byFullOrdinate, not abscissa.
                    spots.sort(org.audiveris.omr.glyph.Glyphs.byFullOrdinate);

                    for (Glyph glyph : spots) {
                        final Rectangle box = glyph.getBounds();
                        System.out.println("spot " + index + " system " + system.getId() + " "
                                + box.x + " " + box.y + " " + box.width + " " + box.height + " "
                                + glyph.getWeight());

                        final String reason = describe(
                                glyph, spotLag, itemParams, constants, scale, index);
                        if (reason != null) {
                            System.out.println("reject " + index + " " + reason);
                        }

                        index++;
                    }
                }
            }
        }
    }

    /**
     * Replays {@code checkBeamGlyph} on one spot, printing what it found.
     *
     * @param glyph      the spot
     * @param spotLag    the spot lag
     * @param itemParams the item parameters
     * @param constants  BeamsBuilder's constant set
     * @param scale      sheet scale
     * @param index      spot index, for the record keys
     * @return the refusal reason, or null if the spot produced a structure
     * @throws Exception if reflection fails
     */
    private static String describe (Glyph glyph,
                                    Lag spotLag,
                                    Object itemParams,
                                    Object constants,
                                    Scale scale,
                                    int index)
        throws Exception
    {
        final Rectangle box = glyph.getBounds();
        final double minBeamWidthLow = (Double) field(itemParams, "minBeamWidthLow");
        final double minHeightLow = (Double) field(itemParams, "minHeightLow");
        final double maxBeamSlope = ((org.audiveris.omr.constant.Constant.Double) field(
                constants, "maxBeamSlope")).getValue();
        final double maxDistanceToBorder = scale.toPixelsDouble(
                (Scale.Fraction) field(constants, "maxDistanceToBorder"));
        final double maxJitterRatio = ((org.audiveris.omr.constant.Constant.Ratio) field(
                constants, "maxJitterRatio")).getValue();

        if (box.width < minBeamWidthLow) {
            return "too narrow";
        }

        final double meanHeight = glyph.getMeanThickness(Orientation.HORIZONTAL);
        System.out.println(String.format("meanheight %d %.9f", index, meanHeight));

        if (meanHeight < minHeightLow) {
            return "too slim";
        }

        try {
            if (Math.abs(LineUtil.getSlope(glyph.getCenterLine())) > maxBeamSlope) {
                return "too steep";
            }
        } catch (Exception ignored) {
            return "vertical";
        }

        final BeamStructure structure = new BeamStructure(glyph, spotLag,
                (org.audiveris.omr.sheet.beam.BeamsBuilder.ItemParameters) itemParams);
        final Double meanDist = structure.computeLines();
        System.out.println("meandist " + index + " " + meanDist);

        if ((meanDist == null) || (meanDist > maxDistanceToBorder)) {
            return "wavy or inconsistent borders";
        }

        final double structWidth = structure.getWidth();
        System.out.println(String.format("structwidth %d %.9f", index, structWidth));

        if (structWidth < minBeamWidthLow) {
            return "too narrow width";
        }

        final double lineSlopeGap = structure.compareSlopes();
        System.out.println(String.format("slopegap %d %.9f", index, lineSlopeGap));

        if (lineSlopeGap > org.audiveris.omr.sig.inter.BeamGroupInter.getMaxSlopeDiff()) {
            return "diverging beams";
        }

        // The sections retrieveItems walks, dumped against the median it
        // actually used -- the one from computeLines, *before* adjustSides.
        // Dumping the adjusted median instead answers a question nobody asked.
        //
        // y is printed at full precision because that is the whole point: the
        // containment test is half-open, so a y of 924.9999999999 is inside a
        // run ending at 924 and a y of exactly 925.0 is not, and %.9f prints
        // both as 925.000000000.
        @SuppressWarnings("unchecked")
        final List<org.audiveris.omr.lag.Section> sections =
                (List<org.audiveris.omr.lag.Section>) call(
                        structure, "getGlyphSections", new Class<?>[] {});
        final List<?> preLines = structure.getLines();

        for (int l = 0; l < preLines.size(); l++) {
            final Line2D preMedian = (Line2D) field(preLines.get(l), "median");
            System.out.println("premedian " + index + " " + l + " "
                    + preMedian.getX1() + " " + preMedian.getY1() + " "
                    + preMedian.getX2() + " " + preMedian.getY2());

            for (int si = 0; si < sections.size(); si++) {
                final org.audiveris.omr.lag.Section section = sections.get(si);
                final Rectangle sctBox = section.getBounds();
                final java.awt.geom.Point2D sctCenter =
                        org.audiveris.omr.math.GeoUtil.center2D(sctBox);
                final double y = org.audiveris.omr.math.LineUtil.yAtX(
                        preMedian, sctCenter.getX());
                System.out.println("section " + index + " " + l + " " + si + " "
                        + sctBox.x + " " + sctBox.y + " " + sctBox.width + " "
                        + sctBox.height + " " + sctCenter.getX() + " " + y + " "
                        + section.contains(sctCenter.getX(), y));
            }
        }

        structure.adjustSides();
        structure.extendMiddleLines();
        structure.splitLines();

        final List<?> lines = structure.getLines();
        for (int l = 0; l < lines.size(); l++) {
            final Object beamLine = lines.get(l);
            System.out.println(String.format(
                    "line %d %d %s %.9f",
                    index,
                    l,
                    line((Line2D) field(beamLine, "median")),
                    (Double) field(beamLine, "height")));

            final List<?> items = (List<?>) call(beamLine, "getItems", new Class<?>[] {});
            for (int i = 0; i < items.size(); i++) {
                final Object item = items.get(i);
                System.out.println(String.format(
                        "item %d %d %d %s %.9f",
                        index,
                        l,
                        i,
                        line((Line2D) field(item, "median")),
                        (Double) field(item, "height")));
            }
        }

        // The jitter that becomes the sixth impact. Java computes it once per
        // structure from the outermost lines, not per item.
        try {
            final Object top = lines.get(0);
            final Object bottom = lines.get(lines.size() - 1);
            final double topJitter = structure.computeJitter(
                    (org.audiveris.omr.sheet.beam.BeamLine) top,
                    org.audiveris.omr.util.VerticalSide.TOP);
            final double botJitter = structure.computeJitter(
                    (org.audiveris.omr.sheet.beam.BeamLine) bottom,
                    org.audiveris.omr.util.VerticalSide.BOTTOM);
            final double meanJitter = 0.5 * (topJitter + botJitter);
            System.out.println(String.format(
                    "jitter %d %.9f %.9f %.9f",
                    index,
                    topJitter,
                    botJitter,
                    1 - (meanJitter / maxJitterRatio)));
        } catch (Exception ex) {
            System.out.println("jitter " + index + " failed");
        }

        return null;
    }
}
