// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.Rectangle;
import java.awt.geom.Line2D;
import java.awt.geom.Point2D;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.List;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.Glyph;
import org.audiveris.omr.glyph.GlyphGroup;
import org.audiveris.omr.run.Orientation;
import org.audiveris.omr.run.Run;
import org.audiveris.omr.run.RunTable;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sig.GradeImpacts;
import org.audiveris.omr.sig.inter.AbstractBeamInter;
import org.audiveris.omr.sig.inter.BeamGroupInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.MultipleRestInter;
import org.audiveris.omr.step.OmrStep;

/**
 * Exact differential oracle for the production BEAMS use of STEM_SEEDS.
 *
 * <p>Each page is loaded twice and reaches the real STEM_SEEDS step. One copy proceeds through the
 * real BEAMS step unchanged. In the counterfactual copy, only the {@link GlyphGroup#VERTICAL_SEED}
 * membership and corresponding free-glyph entry are removed; registered glyphs and every other
 * piece of sheet state remain intact. BEAMS then runs normally. Since the only BEAMS lookup of
 * VERTICAL_SEED is {@code BeamsBuilder.extendToStem}, any exact state difference is that path's
 * production effect.
 *
 * <p>The fixture stays compact by publishing exact FNV digests of canonical beam, group and
 * multiple-rest records. If either side differs, the full multiset difference is emitted as well.
 * All floating-point values use {@link Double#toHexString(double)}.
 */
public class BeamStemSeedsProbe
{
    private static final Orientation VERTICAL = Orientation.VERTICAL;

    private BeamStemSeedsProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
    {
        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "BEAMS");
        final Field cliField = Main.class.getDeclaredField("cli");
        cliField.setAccessible(true);
        cliField.set(null, cli);
        org.audiveris.omr.ui.symbol.MusicFont.checkMusicFont();

        if ((args.length > 0) && (args[0].equals("--seeded") || args[0].equals("--empty"))) {
            runSingleMode(args);
            System.exit(0);
        }

        printHeader();

        int corpusSheets = 0;
        int corpusSystems = 0;
        int corpusSeeds = 0;
        int corpusSeededBeams = 0;
        int corpusEmptyBeams = 0;
        int corpusSeededGroups = 0;
        int corpusEmptyGroups = 0;
        int corpusChangedSystems = 0;
        int corpusRemoved = 0;
        int corpusAdded = 0;
        final List<String> corpusRows = new ArrayList<>();

        for (String arg : args) {
            final String[] parts = arg.split(":");
            final Path path = Paths.get(parts[0]).toAbsolutePath();
            final int wanted = Integer.parseInt(parts[1]);
            final PageRun seeded = runPage(path, wanted, false);
            final PageRun empty = runPage(path, wanted, true);

            if (seeded.systems.size() != empty.systems.size()) {
                throw new IllegalStateException("seeded and empty page system counts differ");
            }

            final String page = path.getFileName() + "#" + wanted;
            final List<String> sheetRows = new ArrayList<>();
            int sheetSeeds = 0;
            int sheetSeededBeams = 0;
            int sheetEmptyBeams = 0;
            int sheetSeededGroups = 0;
            int sheetEmptyGroups = 0;
            int sheetChangedSystems = 0;
            int sheetRemoved = 0;
            int sheetAdded = 0;
            System.out.printf("beamstemseedsheet %s systems %d%n", page, seeded.systems.size());

            for (int index = 0; index < seeded.systems.size(); index++) {
                final SystemState withSeeds = seeded.systems.get(index);
                final SystemState withoutSeeds = empty.systems.get(index);
                if (withSeeds.systemId != withoutSeeds.systemId) {
                    throw new IllegalStateException("seeded and empty system order differs");
                }
                if (!withSeeds.seeds.equals(withoutSeeds.seeds)) {
                    throw new IllegalStateException(
                            "independent STEM_SEEDS inputs differ in " + page + " system "
                                    + withSeeds.systemId);
                }

                final String input = String.format(
                        "beamstemseedinput %s system %d seeds %d %016x",
                        page,
                        withSeeds.systemId,
                        withSeeds.seeds.size(),
                        hash(withSeeds.seeds));
                System.out.println(input);
                sheetRows.add(input);

                final List<String> removed = multisetDifference(
                        withSeeds.canonicalState(), withoutSeeds.canonicalState());
                final List<String> added = multisetDifference(
                        withoutSeeds.canonicalState(), withSeeds.canonicalState());
                final boolean changed = !removed.isEmpty() || !added.isEmpty();
                final String comparison = String.format(
                        "beamstemseedcompare %s system %d seededBeams %d %016x emptyBeams %d "
                                + "%016x seededGroups %d %016x emptyGroups %d %016x "
                                + "seededRests %d %016x emptyRests %d %016x changed %s "
                                + "removed %d added %d",
                        page,
                        withSeeds.systemId,
                        withSeeds.beams.size(),
                        hash(withSeeds.beams),
                        withoutSeeds.beams.size(),
                        hash(withoutSeeds.beams),
                        withSeeds.groups.size(),
                        hash(withSeeds.groups),
                        withoutSeeds.groups.size(),
                        hash(withoutSeeds.groups),
                        withSeeds.rests.size(),
                        hash(withSeeds.rests),
                        withoutSeeds.rests.size(),
                        hash(withoutSeeds.rests),
                        changed,
                        removed.size(),
                        added.size());
                System.out.println(comparison);
                sheetRows.add(comparison);

                for (String record : removed) {
                    final String row = "beamstemseedremoved " + page + " system "
                            + withSeeds.systemId + " " + record;
                    System.out.println(row);
                    sheetRows.add(row);
                }
                for (String record : added) {
                    final String row = "beamstemseedadded " + page + " system "
                            + withSeeds.systemId + " " + record;
                    System.out.println(row);
                    sheetRows.add(row);
                }

                sheetSeeds += withSeeds.seeds.size();
                sheetSeededBeams += withSeeds.beams.size();
                sheetEmptyBeams += withoutSeeds.beams.size();
                sheetSeededGroups += withSeeds.groups.size();
                sheetEmptyGroups += withoutSeeds.groups.size();
                sheetChangedSystems += changed ? 1 : 0;
                sheetRemoved += removed.size();
                sheetAdded += added.size();
            }

            System.out.printf(
                    "beamstemseedsheetsummary %s systems %d seeds %d seededBeams %d "
                            + "emptyBeams %d seededGroups %d emptyGroups %d changedSystems %d "
                            + "removed %d added %d %016x%n",
                    page,
                    seeded.systems.size(),
                    sheetSeeds,
                    sheetSeededBeams,
                    sheetEmptyBeams,
                    sheetSeededGroups,
                    sheetEmptyGroups,
                    sheetChangedSystems,
                    sheetRemoved,
                    sheetAdded,
                    hash(sheetRows));
            corpusSheets++;
            corpusSystems += seeded.systems.size();
            corpusSeeds += sheetSeeds;
            corpusSeededBeams += sheetSeededBeams;
            corpusEmptyBeams += sheetEmptyBeams;
            corpusSeededGroups += sheetSeededGroups;
            corpusEmptyGroups += sheetEmptyGroups;
            corpusChangedSystems += sheetChangedSystems;
            corpusRemoved += sheetRemoved;
            corpusAdded += sheetAdded;
            corpusRows.addAll(sheetRows);
        }

        System.out.printf(
                "beamstemseedcorpussummary sheets %d systems %d seeds %d seededBeams %d "
                        + "emptyBeams %d seededGroups %d emptyGroups %d changedSystems %d "
                        + "removed %d added %d %016x%n",
                corpusSheets,
                corpusSystems,
                corpusSeeds,
                corpusSeededBeams,
                corpusEmptyBeams,
                corpusSeededGroups,
                corpusEmptyGroups,
                corpusChangedSystems,
                corpusRemoved,
                corpusAdded,
                hash(corpusRows));
        System.exit(0);
    }

    private static void runSingleMode (String[] args)
        throws Exception
    {
        final boolean removeSeeds = args[0].equals("--empty");
        final String mode = removeSeeds ? "empty" : "seeded";
        System.out.println("# Java Audiveris 5.11 (Temurin JDK 25) BEAMS/STEM_SEEDS state.");
        System.out.println("# Mode: " + mode);
        System.out.println("# Run seeded and empty in separate JVMs. Apart from this mode header,");
        System.out.println("# byte-identical state rows prove actual seeds do not affect BEAMS.");
        System.out.println("# Exact canonical hashes cover materialized seeds, final beams, groups,");
        System.out.println("# multiple rests, all beam impacts, and exact group membership.");
        System.out.println("# Doubles are Double.toHexString; hashes are FNV-1a-64 over rows + newline.");
        System.out.println("#");
        System.out.println("# Regenerate from the repository root with beamStemSeedMode=" + mode + ".");

        int sheets = 0;
        int systems = 0;
        int seeds = 0;
        int beams = 0;
        int groups = 0;
        int rests = 0;
        final List<String> corpusRows = new ArrayList<>();

        for (int argIndex = 1; argIndex < args.length; argIndex++) {
            final String[] parts = args[argIndex].split(":");
            final Path path = Paths.get(parts[0]).toAbsolutePath();
            final int wanted = Integer.parseInt(parts[1]);
            final PageRun run = runPage(path, wanted, removeSeeds);
            final String page = path.getFileName() + "#" + wanted;
            for (SystemState state : run.systems) {
                final String row = String.format(
                        "beamstemseedstate %s system %d seeds %d %016x beams %d %016x "
                                + "groups %d %016x rests %d %016x state %016x",
                        page,
                        state.systemId,
                        state.seeds.size(),
                        hash(state.seeds),
                        state.beams.size(),
                        hash(state.beams),
                        state.groups.size(),
                        hash(state.groups),
                        state.rests.size(),
                        hash(state.rests),
                        hash(state.canonicalState()));
                System.out.println(row);
                corpusRows.add(row);
                systems++;
                seeds += state.seeds.size();
                beams += state.beams.size();
                groups += state.groups.size();
                rests += state.rests.size();
            }
            sheets++;
        }

        System.out.printf(
                "beamstemseedstatesummary sheets %d systems %d seeds %d beams %d groups %d "
                        + "rests %d %016x%n",
                sheets,
                systems,
                seeds,
                beams,
                groups,
                rests,
                hash(corpusRows));
    }

    private static PageRun runPage (Path path,
                                    int wanted,
                                    boolean removeSeeds)
        throws Exception
    {
        final Book book = new Book(path);
        book.createStubs();

        for (SheetStub stub : book.getValidStubs()) {
            if (stub.getNumber() != wanted) {
                continue;
            }

            stub.reachStep(OmrStep.STEM_SEEDS, false);
            final Sheet sheet = stub.getSheet();
            final List<List<String>> seedRecords = new ArrayList<>();
            final List<List<Glyph>> seedsBySystem = new ArrayList<>();

            // Snapshot all systems before mutating any glyph. GlyphIndex can return the same
            // registered glyph object to adjacent systems, so clearing one system first would
            // silently alter the observed seed input of a later system.
            for (SystemInfo system : sheet.getSystems()) {
                final List<Glyph> seeds = new ArrayList<>(
                        system.getGroupedGlyphs(GlyphGroup.VERTICAL_SEED));
                seedsBySystem.add(seeds);
                final List<String> records = new ArrayList<>();
                for (Glyph seed : seeds) {
                    records.add(seedRecord(seed));
                }
                Collections.sort(records);
                seedRecords.add(records);
            }

            if (removeSeeds) {
                for (int index = 0; index < sheet.getSystems().size(); index++) {
                    final SystemInfo system = sheet.getSystems().get(index);
                    final List<Glyph> seeds = seedsBySystem.get(index);
                    for (Glyph seed : seeds) {
                        seed.getGroups().remove(GlyphGroup.VERTICAL_SEED);
                        if (seed.getGroups().isEmpty()) {
                            system.removeFreeGlyph(seed);
                        }
                    }
                    if (!system.getGroupedGlyphs(GlyphGroup.VERTICAL_SEED).isEmpty()) {
                        throw new IllegalStateException("failed to hide every vertical seed");
                    }
                }
            }

            stub.reachStep(OmrStep.BEAMS, false);
            final List<SystemState> states = new ArrayList<>();
            for (int index = 0; index < sheet.getSystems().size(); index++) {
                states.add(systemState(sheet.getSystems().get(index), seedRecords.get(index)));
            }
            return new PageRun(states);
        }

        throw new IllegalArgumentException("page " + wanted + " not found in " + path);
    }

    private static SystemState systemState (SystemInfo system,
                                            List<String> seeds)
    {
        final List<String> beams = new ArrayList<>();
        for (Inter inter : system.getSig().inters(AbstractBeamInter.class)) {
            beams.add(beamRecord((AbstractBeamInter) inter));
        }
        Collections.sort(beams);

        final List<String> groups = new ArrayList<>();
        for (Inter inter : system.getSig().inters(BeamGroupInter.class)) {
            final BeamGroupInter group = (BeamGroupInter) inter;
            final List<String> members = new ArrayList<>();
            for (AbstractBeamInter beam : group.getBeams()) {
                members.add(beamRecord(beam));
            }
            Collections.sort(members);
            groups.add("group members " + members.size() + " " + String.join(";", members));
        }
        Collections.sort(groups);

        final List<String> rests = new ArrayList<>();
        for (Inter inter : system.getSig().inters(MultipleRestInter.class)) {
            final MultipleRestInter rest = (MultipleRestInter) inter;
            rests.add(horizontalRecord("rest", rest.getClass().getSimpleName(),
                    rest.getShape().toString(), rest.getMedian(), rest.getHeight(),
                    rest.getGrade(), rest.getImpacts()));
        }
        Collections.sort(rests);

        return new SystemState(system.getId(), seeds, beams, groups, rests);
    }

    private static String beamRecord (AbstractBeamInter beam)
    {
        return horizontalRecord(
                "beam",
                beam.getClass().getSimpleName(),
                beam.getShape().toString(),
                beam.getMedian(),
                beam.getHeight(),
                beam.getGrade(),
                beam.getImpacts());
    }

    private static String horizontalRecord (String tag,
                                            String type,
                                            String shape,
                                            Line2D median,
                                            double height,
                                            double grade,
                                            GradeImpacts impacts)
    {
        final StringBuilder record = new StringBuilder(tag).append(' ').append(type).append(' ')
                .append(shape).append(" median ").append(hex(median.getX1())).append(' ')
                .append(hex(median.getY1())).append(' ').append(hex(median.getX2())).append(' ')
                .append(hex(median.getY2())).append(" height ").append(hex(height))
                .append(" grade ").append(hex(grade));
        if (impacts == null) {
            record.append(" impacts none");
        } else {
            record.append(" impacts");
            for (int index = 0; index < impacts.getImpactCount(); index++) {
                record.append(' ').append(impacts.getName(index)).append(':')
                        .append(hex(impacts.getWeight(index))).append(':')
                        .append(hex(impacts.getImpact(index)));
            }
        }
        return record.toString();
    }

    private static String seedRecord (Glyph seed)
    {
        final Rectangle box = seed.getBounds();
        final Point2D start = seed.getStartPoint(VERTICAL);
        final Point2D stop = seed.getStopPoint(VERTICAL);
        return String.format(
                "seed bounds %d %d %d %d weight %d start %s %s stop %s %s thickness %s "
                        + "distance %s runs %016x",
                box.x,
                box.y,
                box.width,
                box.height,
                seed.getWeight(),
                hex(start.getX()),
                hex(start.getY()),
                hex(stop.getX()),
                hex(stop.getY()),
                hex(seed.getMeanThickness(VERTICAL)),
                hex(seed.getMeanDistance()),
                runTableHash(seed.getRunTable()));
    }

    private static long runTableHash (RunTable table)
    {
        final List<String> records = new ArrayList<>();
        records.add(String.format(
                "%s %d %d", table.getOrientation(), table.getWidth(), table.getHeight()));
        for (int sequence = 0; sequence < table.getSize(); sequence++) {
            final StringBuilder record = new StringBuilder().append(sequence);
            for (java.util.Iterator<Run> it = table.iterator(sequence); it.hasNext();) {
                final Run run = it.next();
                record.append(' ').append(run.getStart()).append(':').append(run.getLength());
            }
            records.add(record.toString());
        }
        return hash(records);
    }

    private static List<String> multisetDifference (List<String> minuend,
                                                    List<String> subtrahend)
    {
        final List<String> difference = new ArrayList<>(minuend);
        for (String record : subtrahend) {
            difference.remove(record);
        }
        Collections.sort(difference);
        return difference;
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

    private static void printHeader ()
    {
        System.out.println("# Java Audiveris 5.11 (Temurin JDK 25) BEAMS/STEM_SEEDS effect.");
        System.out.println("# Each page runs the real STEM_SEEDS and BEAMS steps twice. The seeded");
        System.out.println("# run is untouched. The empty run removes only VERTICAL_SEED free-glyph");
        System.out.println("# visibility after STEM_SEEDS; registered glyphs and all other state stay.");
        System.out.println("# BEAMS reads VERTICAL_SEED only in BeamsBuilder.extendToStem.");
        System.out.println("#");
        System.out.println("# Beam/group/rest digests cover exact class, shape, median, height, grade,");
        System.out.println("# impacts and group membership. Differences emit their complete records.");
        System.out.println("# Seed digests cover geometry and every run of each materialized glyph.");
        System.out.println("# Doubles are Double.toHexString; hashes are FNV-1a-64 over rows + newline.");
        System.out.println("#");
        System.out.println("# Regenerate from the repository root:");
        System.out.println(
                "#   env -u JAVA_TOOL_OPTIONS JAVA_HOME=/path/to/jdk25/Contents/Home \\");
        System.out.println("#     ./gradlew --no-daemon -q \\");
        System.out.println(
                "#     -I rust/oracle/java/staff-impacts.init.gradle "
                        + ":app:beamStemSeedsProbe \\");
        System.out.println("#     '-PbeamStemSeedTargets=data/examples/chula.png:1 ...'");
    }

    private static class PageRun
    {
        final List<SystemState> systems;

        PageRun (List<SystemState> systems)
        {
            this.systems = systems;
        }
    }

    private static class SystemState
    {
        final int systemId;

        final List<String> seeds;

        final List<String> beams;

        final List<String> groups;

        final List<String> rests;

        SystemState (int systemId,
                     List<String> seeds,
                     List<String> beams,
                     List<String> groups,
                     List<String> rests)
        {
            this.systemId = systemId;
            this.seeds = seeds;
            this.beams = beams;
            this.groups = groups;
            this.rests = rests;
        }

        List<String> canonicalState ()
        {
            final List<String> records = new ArrayList<>();
            records.addAll(beams);
            records.addAll(groups);
            records.addAll(rests);
            Collections.sort(records);
            return records;
        }
    }
}
