// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.Point;
import java.awt.Rectangle;
import java.awt.geom.Point2D;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.constant.Constant;
import org.audiveris.omr.constant.ConstantSet;
import org.audiveris.omr.glyph.dynamic.StraightFilament;
import org.audiveris.omr.lag.Section;
import org.audiveris.omr.run.Orientation;
import org.audiveris.omr.run.Run;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.stem.StemScaler;
import org.audiveris.omr.sheet.stem.VerticalsBuilder;
import org.audiveris.omr.step.OmrStep;

/**
 * Exact raw {@link VerticalsBuilder} candidate oracle.
 *
 * <p>The production STEM_SEEDS step immediately checks and materializes the filaments returned by
 * {@code retrieveCandidates()}, which makes its final glyphs too late an oracle for porting
 * {@code StickFactory}. This probe reaches HEADERS, installs the production stem scale, and calls
 * only that private candidate method. It deliberately never calls {@code checkVerticals()}.
 *
 * <p>Selected section digests cover every run, in Java's system-section order. Candidate rows stay
 * in factory order, and member ids stay in {@code SectionCompound}'s mixed-orientation order.
 * Doubles use {@link Double#toHexString(double)} so no printed decimal boundary is ambiguous.
 */
public class StemSeedsProbe
{
    private static final Orientation VERTICAL = Orientation.VERTICAL;

    private StemSeedsProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
    {
        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "HEADERS");
        final Field cliField = Main.class.getDeclaredField("cli");
        cliField.setAccessible(true);
        cliField.set(null, cli);
        org.audiveris.omr.ui.symbol.MusicFont.checkMusicFont();

        final Method retrieve = VerticalsBuilder.class.getDeclaredMethod("retrieveCandidates");
        retrieve.setAccessible(true);

        printHeader();

        int corpusSheets = 0;
        int corpusSystems = 0;
        int corpusCandidates = 0;
        final List<String> corpusRows = new ArrayList<>();

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

                stub.reachStep(OmrStep.HEADERS, false);
                final Sheet sheet = stub.getSheet();
                final Scale scale = sheet.getScale();
                scale.setStemScale(new StemScaler(sheet).retrieveStemWidth());

                final String page = path.getFileName() + "#" + wanted;
                final List<String> sheetRows = new ArrayList<>();
                int sheetCandidates = 0;

                System.out.printf("stemseedsheet %s systems %d%n", page, sheet.getSystems().size());

                for (SystemInfo system : sheet.getSystems()) {
                    final List<Section> verticals = selectedVerticals(system);
                    final List<Section> stickers = selectedStickers(system);
                    final int minCore = minimumCoreLength(system);
                    final String input = String.format(
                            "stemseedinput %s system %d profile %d left %d right %d interline %d "
                                    + "maxStem %d minCore %d minSide %s vertical %d %016x "
                                    + "stickers %d %016x",
                            page,
                            system.getId(),
                            system.getProfile(),
                            system.getLeft(),
                            system.getRight(),
                            scale.getInterline(),
                            scale.getMaxStem(),
                            minCore,
                            hex(VerticalsBuilder.getMinSideRatio().getValue()),
                            verticals.size(),
                            sectionHash(verticals),
                            stickers.size(),
                            sectionHash(stickers));
                    System.out.println(input);
                    sheetRows.add(input);

                    final VerticalsBuilder builder = new VerticalsBuilder(system);
                    @SuppressWarnings("unchecked")
                    final List<StraightFilament> candidates =
                            (List<StraightFilament>) retrieve.invoke(builder);
                    final List<String> candidateRows = new ArrayList<>();

                    for (int ordinal = 0; ordinal < candidates.size(); ordinal++) {
                        final String record = candidateRecord(
                                page, system.getId(), ordinal, candidates.get(ordinal));
                        candidateRows.add(record);
                        sheetRows.add(record);
                        System.out.println(record);
                    }

                    System.out.printf(
                            "stemseedcandidatesummary %s system %d count %d %016x%n",
                            page,
                            system.getId(),
                            candidateRows.size(),
                            hash(candidateRows));
                    sheetCandidates += candidateRows.size();
                }

                System.out.printf(
                        "stemseedsheetsummary %s systems %d candidates %d %016x%n",
                        page,
                        sheet.getSystems().size(),
                        sheetCandidates,
                        hash(sheetRows));
                corpusSheets++;
                corpusSystems += sheet.getSystems().size();
                corpusCandidates += sheetCandidates;
                corpusRows.addAll(sheetRows);
            }
        }

        System.out.printf(
                "stemseedcorpussummary sheets %d systems %d candidates %d %016x%n",
                corpusSheets,
                corpusSystems,
                corpusCandidates,
                hash(corpusRows));
        System.exit(0);
    }

    private static void printHeader ()
    {
        System.out.println("# Java Audiveris 5.11 (Temurin JDK 25) raw STEM_SEEDS candidates.");
        System.out.println("# Generated by StemSeedsProbe from HEADERS plus StemScaler; the probe");
        System.out.println("# reflectively calls VerticalsBuilder.retrieveCandidates() and never");
        System.out.println("# calls checkVerticals(), so these are StickFactory outputs, not seeds.");
        System.out.println("#");
        System.out.println("# stemseedinput section digests cover tag/id, position, bounds, weight,");
        System.out.println("# and every run in Java order. Candidate members retain the compound's");
        System.out.println("# mixed-orientation order; v/h tags disambiguate the two lag id spaces.");
        System.out.println("# All floating-point values are Double.toHexString exact encodings.");
        System.out.println("# Summary digests are FNV-1a-64 over exact record rows plus newline.");
        System.out.println("#");
        System.out.println("# Regenerate from the repository root:");
        System.out.println("#   env -u JAVA_TOOL_OPTIONS JAVA_HOME=/path/to/jdk25/Contents/Home \\");
        System.out.println("#     ./gradlew --no-daemon -q \\");
        System.out.println("#     -I rust/oracle/java/staff-impacts.init.gradle :app:stemSeedsProbe \\");
        System.out.println("#     '-PstemSeedTargets=data/examples/chula.png:1 ...'");
    }

    private static String candidateRecord (String page,
                                           int system,
                                           int ordinal,
                                           StraightFilament candidate)
    {
        final StringBuilder members = new StringBuilder();
        for (Section section : candidate.getMembers()) {
            if (!members.isEmpty()) {
                members.append(',');
            }
            members.append(section.isVertical() ? 'v' : 'h').append(section.getId());
        }

        final Rectangle box = candidate.getBounds();
        final Point2D start = candidate.getStartPoint(VERTICAL);
        final Point2D stop = candidate.getStopPoint(VERTICAL);

        return String.format(
                "stemseedcandidate %s system %d ordinal %d members %s bounds %d %d %d %d "
                        + "weight %d start %s %s stop %s %s thickness %s distance %s",
                page,
                system,
                ordinal,
                members,
                box.x,
                box.y,
                box.width,
                box.height,
                candidate.getWeight(),
                hex(start.getX()),
                hex(start.getY()),
                hex(stop.getX()),
                hex(stop.getY()),
                hex(candidate.getMeanThickness(VERTICAL)),
                hex(candidate.getMeanDistance()));
    }

    private static int minimumCoreLength (SystemInfo system)
        throws Exception
    {
        final Field constantsField = VerticalsBuilder.class.getDeclaredField("constants");
        constantsField.setAccessible(true);
        final ConstantSet constants = (ConstantSet) constantsField.get(null);
        final Field rootField = constants.getClass().getDeclaredField("minCoreSectionLength");
        rootField.setAccessible(true);
        final Constant<?> root = (Constant<?>) rootField.get(constants);
        final Constant<?> selected = constants.getConstant(root, system.getProfile());

        return system.getSheet().getScale().toPixels((Scale.Fraction) selected);
    }

    private static List<Section> selectedVerticals (SystemInfo system)
    {
        final List<Section> selected = new ArrayList<>();
        for (Section section : system.getVerticalSections()) {
            final Point center = section.getAreaCenter();
            if ((center.x > system.getLeft()) && (center.x < system.getRight())) {
                selected.add(section);
            }
        }
        return selected;
    }

    private static List<Section> selectedStickers (SystemInfo system)
    {
        final List<Section> selected = new ArrayList<>();
        for (Section section : system.getHorizontalSections()) {
            if (section.getLength(Orientation.HORIZONTAL) != 1) {
                continue;
            }
            final Point center = section.getAreaCenter();
            if ((center.x > system.getLeft()) && (center.x < system.getRight())) {
                selected.add(section);
            }
        }
        return selected;
    }

    private static long sectionHash (Collection<Section> sections)
    {
        final List<String> records = new ArrayList<>();
        for (Section section : sections) {
            final StringBuilder record = new StringBuilder();
            final Rectangle box = section.getBounds();
            record.append(section.isVertical() ? 'v' : 'h').append(section.getId());
            record.append(' ').append(section.getFirstPos());
            record.append(' ').append(box.x).append(' ').append(box.y);
            record.append(' ').append(box.width).append(' ').append(box.height);
            record.append(' ').append(section.getWeight());
            for (Run run : section.getRuns()) {
                record.append(' ').append(run.getStart()).append(':').append(run.getLength());
            }
            records.add(record.toString());
        }
        return hash(records);
    }

    private static String hex (double value)
    {
        return Double.toHexString(value);
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
}
