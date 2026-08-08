// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import ij.process.ByteProcessor;

import java.awt.Point;
import java.awt.Rectangle;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.Iterator;
import java.util.List;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.Glyph;
import org.audiveris.omr.glyph.GlyphFactory;
import org.audiveris.omr.glyph.Glyphs;
import org.audiveris.omr.image.MorphoProcessor;
import org.audiveris.omr.image.StructureElement;
import org.audiveris.omr.math.Population;
import org.audiveris.omr.run.Orientation;
import org.audiveris.omr.run.Run;
import org.audiveris.omr.run.RunTable;
import org.audiveris.omr.run.RunTableFactory;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Picture;
import org.audiveris.omr.sheet.ProcessingSwitch;
import org.audiveris.omr.sheet.ProcessingSwitches;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.Staff;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.beam.BeamsBuilder;
import org.audiveris.omr.sheet.beam.BlackHeadSizer;
import org.audiveris.omr.sheet.beam.SpotsBuilder;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.ui.symbol.MusicFont;

/**
 * Exact oracle for {@link BlackHeadSizer}, the BEAMS side effect that sizes HEADS templates.
 *
 * <p>Each page first reaches the real STEM_SEEDS step. The probe then calls the production
 * {@link SpotsBuilder#getBuffer} and {@link SpotsBuilder#buildSpots} path. That production call
 * performs the threshold-170 {@code HEAD_SPOTS} save, creates the threshold-140 beam components,
 * and invokes {@code BlackHeadSizer.process} before returning the exact component list it used.
 *
 * <p>The probe replays the private production {@code checkSpot} and {@code closeBlackHead} methods
 * reflectively to expose their otherwise hidden per-component decisions. A second call through the
 * same production morphology, threshold, run-table and glyph-factory classes exposes the component
 * count that {@code closeBlackHead} otherwise collapses to null; the probe asserts both views agree.
 * The scale reported here is the state installed by the original production invocation, not state
 * synthesized by the replay.
 *
 * <p>Run one page per fresh JVM. Audiveris has process-global registries, so a corpus assembled in
 * one JVM would confound recognition state with target ordering.
 */
public class BlackHeadSizerProbe
{
    private BlackHeadSizerProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
    {
        if ((args.length == 1) && args[0].equals("--header")) {
            printHeader();
            System.exit(0);
        }

        if (args.length != 1) {
            throw new IllegalArgumentException("expected exactly one <path>:<sheet> target");
        }

        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "STEM_SEEDS");
        final Field cliField = Main.class.getDeclaredField("cli");
        cliField.setAccessible(true);
        cliField.set(null, cli);
        MusicFont.checkMusicFont();

        final String[] parts = args[0].split(":");
        if (parts.length != 2) {
            throw new IllegalArgumentException("target must be <path>:<sheet>");
        }
        final Path path = Paths.get(parts[0]).toAbsolutePath();
        final int wanted = Integer.parseInt(parts[1]);
        runPage(path, wanted);
        System.exit(0);
    }

    private static void runPage (Path path,
                                 int wanted)
        throws Exception
    {
        final Book book = new Book(path);
        book.createStubs();
        SheetStub wantedStub = null;

        for (SheetStub stub : book.getValidStubs()) {
            if (stub.getNumber() == wanted) {
                wantedStub = stub;
                break;
            }
        }
        if (wantedStub == null) {
            throw new IllegalArgumentException("missing sheet " + wanted + " in " + path);
        }

        wantedStub.reachStep(OmrStep.STEM_SEEDS, false);
        final Sheet sheet = wantedStub.getSheet();
        final Picture picture = sheet.getPicture();
        final Scale scale = sheet.getScale();
        final String page = path.getFileName() + "#" + wanted;
        final ProcessingSwitches switches = wantedStub.getProcessingSwitches();
        final boolean oneLine = switches.getValue(ProcessingSwitch.oneLineStaves);
        final boolean drums = switches.getValue(ProcessingSwitch.drumNotation);
        final boolean eligible = !oneLine && !drums;

        System.out.printf(
                "blackheadpage %s width %d height %d systems %d staves %d interline %d "
                        + "oneLine %s drums %s eligible %s%n",
                page,
                picture.getWidth(),
                picture.getHeight(),
                sheet.getSystems().size(),
                sheet.getStaffManager().getStaffCount(),
                scale.getInterline(),
                oneLine,
                drums,
                eligible);

        Integer beam = scale.getItemValue(Scale.Item.beam);
        if (beam == null) {
            throw new IllegalStateException("missing main beam scale");
        }
        Integer smallBeam = scale.getItemValue(Scale.Item.smallBeam);
        final boolean synthesizeSmall = (smallBeam == null)
                && switches.getValue(ProcessingSwitch.smallBeams);
        if (synthesizeSmall) {
            smallBeam = (int) Math.rint(beam * BeamsBuilder.getCueBeamRatio());
        }
        final int usedBeam = (smallBeam != null) ? Math.min(beam, smallBeam) : beam;

        final SpotsBuilder spotsBuilder = new SpotsBuilder(sheet);
        final ByteProcessor source = (ByteProcessor) call(
                SpotsBuilder.class, spotsBuilder, "getBuffer", new Class<?>[] {});
        System.out.printf(
                "blackheadbuffer %s stage getBuffer width %d height %d %016x%n",
                page,
                source.getWidth(),
                source.getHeight(),
                pixelHash(source));

        // This is the actual production invocation. It saves HEAD_SPOTS, creates the exact
        // threshold-140 component list returned below, and runs BlackHeadSizer.process on it.
        final ByteProcessor beamBinary = (ByteProcessor) source.duplicate();
        final List<Glyph> spots = spotsBuilder.buildSpots(
                beamBinary, null, (double) usedBeam, null);

        final RunTable headRuns = picture.getVerticalTable(Picture.TableKey.HEAD_SPOTS);
        if (headRuns == null) {
            throw new IllegalStateException("buildSpots did not retain HEAD_SPOTS");
        }
        final List<Glyph> headComponents = GlyphFactory.buildGlyphs(headRuns, new Point(0, 0));
        final List<String> headComponentRows = glyphRows("head", headComponents);
        System.out.printf(
                "blackheadheadruns %s orientation %s width %d height %d runs %d weight %d "
                        + "pixel %016x table %016x components %d componentHash %016x%n",
                page,
                headRuns.getOrientation(),
                headRuns.getWidth(),
                headRuns.getHeight(),
                headRuns.getTotalRunCount(),
                headRuns.getWeight(),
                pixelHash(headRuns.getBuffer()),
                runTableHash(headRuns),
                headComponents.size(),
                hash(headComponentRows));

        // buildSpots leaves its argument thresholded at 140. Rebuild the table only to freeze its
        // topology; the exact components consumed by BlackHeadSizer are the returned `spots`.
        final RunTable spotRuns = new RunTableFactory(Orientation.VERTICAL).createTable(beamBinary);
        final List<String> inputRows = glyphRows("spot", spots);
        System.out.printf(
                "blackheadspotruns %s orientation %s width %d height %d runs %d weight %d "
                        + "pixel %016x table %016x components %d componentHash %016x%n",
                page,
                spotRuns.getOrientation(),
                spotRuns.getWidth(),
                spotRuns.getHeight(),
                spotRuns.getTotalRunCount(),
                spotRuns.getWeight(),
                pixelHash(spotRuns.getBuffer()),
                runTableHash(spotRuns),
                spots.size(),
                hash(inputRows));

        final BlackHeadSizer sizer = new BlackHeadSizer(sheet);
        final Parameters params = new Parameters(privateField(sizer, "params"));
        System.out.printf(
                "blackheadparams %s beam %d smallBeam %s synthesizedSmall %s usedBeam %d "
                        + "minWidth %d maxWidth %d minHeight %d maxHeight %d minWeight %d "
                        + "maxWeight %d minStackHeight %d maxStackHeight %d minStackWeight %d "
                        + "maxStackWeight %d diameter %s threshold %d quorum %d%n",
                page,
                beam,
                (smallBeam != null) ? smallBeam : "none",
                synthesizeSmall,
                usedBeam,
                params.minWidth,
                params.maxWidth,
                params.minHeight,
                params.maxHeight,
                params.minWeight,
                params.maxWeight,
                params.minStackHeight,
                params.maxStackHeight,
                params.minStackWeight,
                params.maxStackWeight,
                Double.toHexString(params.diameter),
                params.binarizationThreshold,
                params.singlesQuorum);

        final float radius = (float) (params.diameter - 1) / 2;
        final StructureElement se = new StructureElement(0, 1, radius, new int[] { 0, 0 });
        final MorphoProcessor mp = new MorphoProcessor(se);

        final List<Candidate> singles = new ArrayList<>();
        final List<Candidate> stacks = new ArrayList<>();
        final List<String> candidateRows = new ArrayList<>();
        int preAccepted = 0;
        int closedCount = 0;

        for (int ordinal = 0; ordinal < spots.size(); ordinal++) {
            final Glyph input = spots.get(ordinal);
            final String pre = checkReason(input, params);
            final boolean checked = (boolean) call(
                    BlackHeadSizer.class,
                    sizer,
                    "checkSpot",
                    new Class<?>[] { Glyph.class },
                    input);
            if (checked != pre.equals("pass")) {
                throw new IllegalStateException("checkSpot reason disagrees at " + ordinal);
            }

            Glyph closed = null;
            String close = "skipped";
            int closeComponents = -1;
            String post = "skipped";
            String kind = "reject";
            boolean registered = false;
            if (checked) {
                preAccepted++;
                closeComponents = closeComponentCount(input, params);
                closed = (Glyph) call(
                        BlackHeadSizer.class,
                        sizer,
                        "closeBlackHead",
                        new Class<?>[] { MorphoProcessor.class, Glyph.class },
                        mp,
                        input);
                if (closed == null) {
                    if (closeComponents == 1) {
                        throw new IllegalStateException(
                                "closeBlackHead discarded one component at " + ordinal);
                    }
                    close = "component_count_not_one";
                    post = "skipped";
                } else {
                    if (closeComponents != 1) {
                        throw new IllegalStateException(
                                "closeBlackHead retained " + closeComponents + " components at "
                                        + ordinal);
                    }
                    close = "one_component";
                    closedCount++;
                    registered = registrationEligible(sheet, closed);
                    post = checkReason(closed, params);
                    final boolean rechecked = (boolean) call(
                            BlackHeadSizer.class,
                            sizer,
                            "checkSpot",
                            new Class<?>[] { Glyph.class },
                            closed);
                    if (rechecked != post.equals("pass")) {
                        throw new IllegalStateException(
                                "second checkSpot reason disagrees at " + ordinal);
                    }
                    if (rechecked) {
                        kind = classification(closed, params);
                    }
                }
            }

            final Candidate candidate = new Candidate(ordinal, input, closed, kind);
            if (kind.equals("single")) {
                singles.add(candidate);
            } else if (kind.equals("stack")) {
                stacks.add(candidate);
            }

            final Rectangle inputBox = input.getBounds();
            final Point inputCentroid = input.getCentroid();
            final String closedFields;
            if (closed == null) {
                closedFields = "none";
            } else {
                final Rectangle box = closed.getBounds();
                final Point centroid = closed.getCentroid();
                closedFields = String.format(
                        "%d %d %d %d %d %d %d %d %016x",
                        box.x,
                        box.y,
                        box.width,
                        box.height,
                        closed.getWeight(),
                        centroid.x,
                        centroid.y,
                        closed.getRunTable().getTotalRunCount(),
                        runTableHash(closed.getRunTable()));
            }
            final String row = String.format(
                    "blackheadcandidate %s ordinal %d bounds %d %d %d %d weight %d centroid "
                            + "%d %d runs %d %016x pre %s close %s closeComponents %s closed %s "
                            + "registered %s post %s class %s",
                    page,
                    ordinal,
                    inputBox.x,
                    inputBox.y,
                    inputBox.width,
                    inputBox.height,
                    input.getWeight(),
                    inputCentroid.x,
                    inputCentroid.y,
                    input.getRunTable().getTotalRunCount(),
                    runTableHash(input.getRunTable()),
                    pre,
                    close,
                    (closeComponents >= 0) ? Integer.toString(closeComponents) : "none",
                    closedFields,
                    registered,
                    post,
                    kind);
            candidateRows.add(row);
            System.out.println(row);
        }

        final List<String> singleRows = printClassRows(page, "single", singles);
        final List<String> stackRows = printClassRows(page, "stack", stacks);

        final List<Candidate> sorted = new ArrayList<>(singles);
        Collections.sort(sorted, (left, right) -> Glyphs.byWidth.compare(left.closed, right.closed));
        final int coreStart = sorted.size() / 4;
        final int coreEnd = (3 * sorted.size()) / 4;
        final Population widths = new Population();
        final Population heights = new Population();
        final List<String> sampleRows = new ArrayList<>();
        for (int sortedOrdinal = 0; sortedOrdinal < sorted.size(); sortedOrdinal++) {
            final Candidate candidate = sorted.get(sortedOrdinal);
            final boolean core = (sortedOrdinal >= coreStart) && (sortedOrdinal < coreEnd);
            if (core) {
                widths.includeValue(candidate.closed.getWidth());
                heights.includeValue(candidate.closed.getHeight());
            }
            final String row = String.format(
                    "blackheadsample %s sorted %d source %d width %d height %d core %s",
                    page,
                    sortedOrdinal,
                    candidate.sourceOrdinal,
                    candidate.closed.getWidth(),
                    candidate.closed.getHeight(),
                    core);
            sampleRows.add(row);
            System.out.println(row);
        }

        if (sorted.size() >= params.singlesQuorum) {
            System.out.printf(
                    "blackheadpopulation %s axis width count %d mean %s deviation %s%n",
                    page,
                    widths.getCardinality(),
                    Double.toHexString(widths.getMeanValue()),
                    Double.toHexString(widths.getStandardDeviation()));
            System.out.printf(
                    "blackheadpopulation %s axis height count %d mean %s deviation %s%n",
                    page,
                    heights.getCardinality(),
                    Double.toHexString(heights.getMeanValue()),
                    Double.toHexString(heights.getStandardDeviation()));
        } else {
            System.out.printf(
                    "blackheadpopulation %s axis width count 0 mean none deviation none%n", page);
            System.out.printf(
                    "blackheadpopulation %s axis height count 0 mean none deviation none%n", page);
        }

        final Scale.BlackHeadScale blackScale = scale.getBlackHeadScale();
        if (blackScale == null) {
            System.out.printf(
                    "blackheadscale %s widthMean none widthDeviation none heightMean none "
                            + "heightDeviation none%n",
                    page);
        } else {
            if (sorted.size() < params.singlesQuorum) {
                throw new IllegalStateException("production installed BlackHeadScale below quorum");
            }
            assertBits("width mean", widths.getMeanValue(), blackScale.getWidthMean());
            assertBits("width deviation", widths.getStandardDeviation(), blackScale.getWidthStd());
            assertBits("height mean", heights.getMeanValue(), blackScale.getHeightMean());
            assertBits(
                    "height deviation", heights.getStandardDeviation(), blackScale.getHeightStd());
            System.out.printf(
                    "blackheadscale %s widthMean %s widthDeviation %s heightMean %s "
                            + "heightDeviation %s%n",
                    page,
                    Double.toHexString(blackScale.getWidthMean()),
                    Double.toHexString(blackScale.getWidthStd()),
                    Double.toHexString(blackScale.getHeightMean()),
                    Double.toHexString(blackScale.getHeightStd()));
        }

        final Scale.MusicFontScale fontScale = scale.getMusicFontScale();
        if (fontScale == null) {
            System.out.printf(
                    "blackheadfont %s family %s name none pointSize none%n",
                    page,
                    wantedStub.getMusicFamily().name());
        } else {
            System.out.printf(
                    "blackheadfont %s family %s name %s pointSize %d%n",
                    page,
                    wantedStub.getMusicFamily().name(),
                    fontScale.getName(),
                    fontScale.getPointSize());
        }

        final List<String> staffRows = new ArrayList<>();
        for (SystemInfo system : sheet.getSystems()) {
            for (Staff staff : system.getStaves()) {
                final String row = String.format(
                        "blackheadstaff %s system %d staff %d specificInterline %d headPointSize %d",
                        page,
                        system.getId(),
                        staff.getId(),
                        staff.getSpecificInterline(),
                        staff.getHeadPointSize());
                staffRows.add(row);
                System.out.println(row);
            }
        }

        System.out.printf(
                "blackheadsummary %s candidates %d preAccepted %d closed %d singles %d stacks %d "
                        + "core %d staves %d candidateHash %016x singleHash %016x stackHash %016x "
                        + "sampleHash %016x staffHash %016x%n",
                page,
                spots.size(),
                preAccepted,
                closedCount,
                singles.size(),
                stacks.size(),
                Math.max(0, coreEnd - coreStart),
                staffRows.size(),
                hash(candidateRows),
                hash(singleRows),
                hash(stackRows),
                hash(sampleRows),
                hash(staffRows));
    }

    private static List<String> printClassRows (String page,
                                                String kind,
                                                List<Candidate> candidates)
    {
        final List<String> rows = new ArrayList<>();
        for (int ordinal = 0; ordinal < candidates.size(); ordinal++) {
            final Candidate candidate = candidates.get(ordinal);
            final Glyph glyph = candidate.closed;
            final Rectangle box = glyph.getBounds();
            final String row = String.format(
                    "blackhead%s %s ordinal %d source %d bounds %d %d %d %d weight %d runs %d "
                            + "%016x",
                    kind,
                    page,
                    ordinal,
                    candidate.sourceOrdinal,
                    box.x,
                    box.y,
                    box.width,
                    box.height,
                    glyph.getWeight(),
                    glyph.getRunTable().getTotalRunCount(),
                    runTableHash(glyph.getRunTable()));
            rows.add(row);
            System.out.println(row);
        }
        return rows;
    }

    private static String classification (Glyph glyph,
                                          Parameters params)
    {
        final int height = glyph.getHeight();
        final int weight = glyph.getWeight();
        if ((height <= params.maxHeight) && (weight <= params.maxWeight)) {
            return "single";
        }
        if ((height >= params.minStackHeight) && (weight >= params.minStackWeight)) {
            return "stack";
        }
        return "reject_neither";
    }

    private static String checkReason (Glyph glyph,
                                       Parameters params)
    {
        final int width = glyph.getWidth();
        if (width < params.minWidth) {
            return "width_low";
        }
        if (width > params.maxWidth) {
            return "width_high";
        }
        final int weight = glyph.getWeight();
        if (weight < params.minWeight) {
            return "weight_low";
        }
        if (weight > params.maxStackWeight) {
            return "weight_high";
        }
        final int height = glyph.getHeight();
        if (height < params.minHeight) {
            return "height_low";
        }
        if (height > params.maxStackHeight) {
            return "height_high";
        }
        return "pass";
    }

    private static int closeComponentCount (Glyph spot,
                                            Parameters params)
    {
        final float radius = (float) (params.diameter - 1) / 2;
        final StructureElement se = new StructureElement(0, 1, radius, new int[] { 0, 0 });
        final MorphoProcessor mp = new MorphoProcessor(se);
        final ByteProcessor buffer = spot.getBuffer();
        mp.close(buffer);
        buffer.threshold(params.binarizationThreshold);
        final RunTable table = new RunTableFactory(SpotsBuilder.SPOT_ORIENTATION)
                .createTable(buffer);
        return GlyphFactory.buildGlyphs(table, spot.getTopLeft()).size();
    }

    private static boolean registrationEligible (Sheet sheet,
                                                 Glyph glyph)
    {
        final Point center = glyph.getCentroid();
        final List<SystemInfo> relevants = sheet.getSystemManager().getSystemsOf(center, null);
        for (SystemInfo system : relevants) {
            if ((center.x >= system.getLeft()) && (center.x <= system.getRight())) {
                return true;
            }
        }
        return false;
    }

    private static Object call (Class<?> owner,
                                Object receiver,
                                String name,
                                Class<?>[] signature,
                                Object... arguments)
        throws Exception
    {
        final Method method = owner.getDeclaredMethod(name, signature);
        method.setAccessible(true);
        return method.invoke(receiver, arguments);
    }

    private static Object privateField (Object receiver,
                                        String name)
        throws Exception
    {
        final Field field = receiver.getClass().getDeclaredField(name);
        field.setAccessible(true);
        return field.get(receiver);
    }

    private static int intField (Object receiver,
                                 String name)
        throws Exception
    {
        final Field field = receiver.getClass().getDeclaredField(name);
        field.setAccessible(true);
        return field.getInt(receiver);
    }

    private static double doubleField (Object receiver,
                                       String name)
        throws Exception
    {
        final Field field = receiver.getClass().getDeclaredField(name);
        field.setAccessible(true);
        return field.getDouble(receiver);
    }

    private static List<String> glyphRows (String kind,
                                           List<Glyph> glyphs)
    {
        final List<String> rows = new ArrayList<>();
        for (int ordinal = 0; ordinal < glyphs.size(); ordinal++) {
            final Glyph glyph = glyphs.get(ordinal);
            final Rectangle box = glyph.getBounds();
            final Point centroid = glyph.getCentroid();
            rows.add(String.format(
                    "%s ordinal %d bounds %d %d %d %d weight %d centroid %d %d runs %d %016x",
                    kind,
                    ordinal,
                    box.x,
                    box.y,
                    box.width,
                    box.height,
                    glyph.getWeight(),
                    centroid.x,
                    centroid.y,
                    glyph.getRunTable().getTotalRunCount(),
                    runTableHash(glyph.getRunTable())));
        }
        return rows;
    }

    private static long runTableHash (RunTable table)
    {
        final List<String> records = new ArrayList<>();
        records.add(String.format(
                "%s %d %d", table.getOrientation(), table.getWidth(), table.getHeight()));
        for (int sequence = 0; sequence < table.getSize(); sequence++) {
            final StringBuilder record = new StringBuilder().append(sequence);
            for (Iterator<Run> it = table.iterator(sequence); it.hasNext();) {
                final Run run = it.next();
                record.append(' ').append(run.getStart()).append(':').append(run.getLength());
            }
            records.add(record.toString());
        }
        return hash(records);
    }

    private static long pixelHash (ByteProcessor buffer)
    {
        long hash = 0xcbf29ce484222325L;
        for (int i = 0, size = buffer.getWidth() * buffer.getHeight(); i < size; i++) {
            hash = (hash ^ (buffer.get(i) & 0xff)) * 0x100000001b3L;
        }
        return hash;
    }

    private static long hash (Collection<String> records)
    {
        long hash = 0xcbf29ce484222325L;
        for (String record : records) {
            for (byte value : (record + "\n").getBytes(StandardCharsets.UTF_8)) {
                hash ^= Byte.toUnsignedLong(value);
                hash *= 0x100000001b3L;
            }
        }
        return hash;
    }

    private static void assertBits (String label,
                                    double expected,
                                    double actual)
    {
        if (Double.doubleToLongBits(expected) != Double.doubleToLongBits(actual)) {
            throw new IllegalStateException(
                    label + " mismatch: " + Double.toHexString(expected) + " != "
                            + Double.toHexString(actual));
        }
    }

    private static void printHeader ()
    {
        System.out.println("# Java Audiveris 5.11 (Temurin JDK 25) BlackHeadSizer oracle.");
        System.out.println("#");
        System.out.println("# Every page reaches the real STEM_SEEDS step in a fresh JVM, then");
        System.out.println("# production SpotsBuilder.buildSpots performs its BEAMS raster work and");
        System.out.println("# invokes BlackHeadSizer.process. HEAD_SPOTS is the threshold-170 table;");
        System.out.println("# BlackHeadSizer consumes the separately listed threshold-140 components.");
        System.out.println("#");
        System.out.println("# Pixel digests are FNV-1a-64 in row-major byte order. Run-table and row");
        System.out.println("# digests are FNV-1a-64 over UTF-8 canonical records with trailing newline.");
        System.out.println("# Components, singles and stacks retain source order. Samples use Java's");
        System.out.println("# stable width sort, and core=true names the [n/4, 3n/4) middle half.");
        System.out.println("# Population and scale doubles use Double.toHexString.");
        System.out.println("#");
        System.out.println("# Compile and run from app/ with the frozen JDK and saved runtime classpath.");
        System.out.println("# Generate one target per fresh JVM, in this order: chula, allegretto,");
        System.out.println("# batuque, carmen, cucaracha, hove, zizi, BachInvention5.");
    }

    private static class Candidate
    {
        final int sourceOrdinal;

        final Glyph input;

        final Glyph closed;

        final String kind;

        Candidate (int sourceOrdinal,
                   Glyph input,
                   Glyph closed,
                   String kind)
        {
            this.sourceOrdinal = sourceOrdinal;
            this.input = input;
            this.closed = closed;
            this.kind = kind;
        }
    }

    private static class Parameters
    {
        final int minWidth;

        final int maxWidth;

        final int minHeight;

        final int maxHeight;

        final int minWeight;

        final int maxWeight;

        final int minStackHeight;

        final int maxStackHeight;

        final int minStackWeight;

        final int maxStackWeight;

        final double diameter;

        final int binarizationThreshold;

        final int singlesQuorum;

        Parameters (Object params)
            throws Exception
        {
            minWidth = intField(params, "minWidth");
            maxWidth = intField(params, "maxWidth");
            minHeight = intField(params, "minHeight");
            maxHeight = intField(params, "maxHeight");
            minWeight = intField(params, "minWeight");
            maxWeight = intField(params, "maxWeight");
            minStackHeight = intField(params, "minStackHeight");
            maxStackHeight = intField(params, "maxStackHeight");
            minStackWeight = intField(params, "minStackWeight");
            maxStackWeight = intField(params, "maxStackWeight");
            diameter = doubleField(params, "diameter");
            binarizationThreshold = intField(params, "binarizationThreshold");
            singlesQuorum = intField(params, "singlesQuorum");
        }
    }
}
