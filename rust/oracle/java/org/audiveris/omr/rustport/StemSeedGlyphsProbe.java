// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.Rectangle;
import java.awt.geom.Point2D;
import java.lang.reflect.Constructor;
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
import org.audiveris.omr.check.Check;
import org.audiveris.omr.check.CheckSuite;
import org.audiveris.omr.glyph.Glyph;
import org.audiveris.omr.glyph.GlyphGroup;
import org.audiveris.omr.glyph.NearLine;
import org.audiveris.omr.glyph.dynamic.StraightFilament;
import org.audiveris.omr.run.Orientation;
import org.audiveris.omr.run.Run;
import org.audiveris.omr.run.RunTable;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.Staff;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.stem.StemChecker;
import org.audiveris.omr.sheet.stem.StemScaler;
import org.audiveris.omr.sheet.stem.VerticalsBuilder;
import org.audiveris.omr.sig.GradeImpacts;
import org.audiveris.omr.step.OmrStep;

/**
 * Exact accepted/materialized {@link VerticalsBuilder} seed-glyph oracle.
 *
 * <p>This is the production boundary immediately after the raw candidates frozen by
 * {@link StemSeedsProbe}. The probe reaches HEADERS, installs the production stem scale, obtains
 * the raw candidates, and invokes {@code VerticalsBuilder.checkVerticals(Collection)} itself.
 * Accepted glyphs are read back from the system's free-glyph set rather than reconstructed here.
 *
 * <p>For every production-checked candidate, the probe also evaluates the real private
 * {@code SeedCheckSuite} checks in their declared order, retaining exact raw values and the
 * {@link GradeImpacts} emitted by {@link StemChecker#checkStem(NearLine)}. This exposes the Clean
 * check's black/white/gap side effects without maintaining a second checker implementation.
 */
public class StemSeedGlyphsProbe
{
    private static final Orientation VERTICAL = Orientation.VERTICAL;

    private static final Method RETRIEVE;

    private static final Method CHECK_VERTICALS;

    private static final Field STEM_CHECKER;

    private static final Method GET_SUITE;

    private static final Constructor<?> STICK_CONTEXT;

    private static final Field CONTEXT_GAP;

    private static final Field CONTEXT_BLACK;

    private static final Field CONTEXT_WHITE;

    static {
        try {
            RETRIEVE = VerticalsBuilder.class.getDeclaredMethod("retrieveCandidates");
            RETRIEVE.setAccessible(true);
            CHECK_VERTICALS = VerticalsBuilder.class.getDeclaredMethod(
                    "checkVerticals", Collection.class);
            CHECK_VERTICALS.setAccessible(true);
            STEM_CHECKER = VerticalsBuilder.class.getDeclaredField("stemChecker");
            STEM_CHECKER.setAccessible(true);
            GET_SUITE = StemChecker.class.getDeclaredMethod("getSuite", int.class);
            GET_SUITE.setAccessible(true);

            final Class<?> contextClass = Class.forName(
                    "org.audiveris.omr.sheet.stem.StemChecker$StickContext");
            STICK_CONTEXT = contextClass.getDeclaredConstructor(NearLine.class);
            STICK_CONTEXT.setAccessible(true);
            CONTEXT_GAP = contextClass.getDeclaredField("gap");
            CONTEXT_BLACK = contextClass.getDeclaredField("black");
            CONTEXT_WHITE = contextClass.getDeclaredField("white");
            CONTEXT_GAP.setAccessible(true);
            CONTEXT_BLACK.setAccessible(true);
            CONTEXT_WHITE.setAccessible(true);
        } catch (ReflectiveOperationException ex) {
            throw new ExceptionInInitializerError(ex);
        }
    }

    private StemSeedGlyphsProbe ()
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

        printHeader();

        int corpusSheets = 0;
        int corpusSystems = 0;
        int corpusCandidates = 0;
        int corpusChecked = 0;
        int corpusAccepted = 0;
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
                int sheetChecked = 0;
                int sheetAccepted = 0;

                System.out.printf("stemseedacceptsheet %s systems %d%n", page,
                        sheet.getSystems().size());

                for (SystemInfo system : sheet.getSystems()) {
                    final VerticalsBuilder builder = new VerticalsBuilder(system);
                    final StemChecker checker = (StemChecker) STEM_CHECKER.get(builder);
                    @SuppressWarnings("unchecked")
                    final List<StraightFilament> candidates =
                            (List<StraightFilament>) RETRIEVE.invoke(builder);
                    final List<CandidateCheck> checks = new ArrayList<>();
                    int checked = 0;

                    for (int ordinal = 0; ordinal < candidates.size(); ordinal++) {
                        final StraightFilament candidate = candidates.get(ordinal);
                        final Point2D center = candidate.getCenter2D();
                        final Staff staff = system.getClosestStaff(center);
                        final String reason;

                        if (staff == null) {
                            reason = "no-staff";
                        } else if (staff.isTablature()) {
                            reason = "tablature";
                        } else if (center.getX() < staff.getHeaderStop()) {
                            reason = "header";
                        } else {
                            reason = "checked";
                            checked++;
                        }

                        checks.add(new CandidateCheck(
                                ordinal,
                                center,
                                staff,
                                reason,
                                reason.equals("checked")
                                        ? inspectChecker(checker, candidate, system.getProfile())
                                        : null));
                    }

                    // Invoke the exact production acceptance/materialization method once, with the
                    // exact factory collection and source order.
                    CHECK_VERTICALS.invoke(builder, candidates);

                    final List<Glyph> accepted = system.getGroupedGlyphs(GlyphGroup.VERTICAL_SEED);
                    final boolean[] acceptedOrdinals = new boolean[candidates.size()];
                    final List<Integer> glyphOrdinals = new ArrayList<>();

                    for (Glyph glyph : accepted) {
                        int found = -1;
                        for (int ordinal = 0; ordinal < candidates.size(); ordinal++) {
                            if (glyph.equals(candidates.get(ordinal).toGlyph(null))) {
                                if (found != -1) {
                                    throw new IllegalStateException(
                                            "materialized glyph matches multiple raw candidates");
                                }
                                found = ordinal;
                            }
                        }
                        if (found == -1) {
                            throw new IllegalStateException(
                                    "materialized glyph has no raw candidate in " + page + " system "
                                            + system.getId());
                        }
                        acceptedOrdinals[found] = true;
                        glyphOrdinals.add(found);
                    }

                    final String input = String.format(
                            "stemseedacceptinput %s system %d profile %d minThreshold %s "
                                    + "candidates %d checked %d accepted %d",
                            page,
                            system.getId(),
                            system.getProfile(),
                            hex(checker.getMinThreshold()),
                            candidates.size(),
                            checked,
                            accepted.size());
                    System.out.println(input);
                    final List<String> systemRows = new ArrayList<>();
                    systemRows.add(input);

                    for (CandidateCheck check : checks) {
                        final boolean acceptedCandidate = acceptedOrdinals[check.ordinal];
                        final String record = check.record(
                                page, system.getId(), checker.getMinThreshold(), acceptedCandidate);
                        System.out.println(record);
                        systemRows.add(record);

                        if (check.details != null) {
                            final boolean gradeAccepted =
                                    check.details.impacts.getGrade() >= checker.getMinThreshold();
                            if (gradeAccepted != acceptedCandidate) {
                                throw new IllegalStateException(
                                        "checker/materialization mismatch in " + page + " system "
                                                + system.getId() + " ordinal " + check.ordinal);
                            }
                        } else if (acceptedCandidate) {
                            throw new IllegalStateException(
                                    "production accepted a staff/header-gated candidate");
                        }
                    }

                    for (int glyphIndex = 0; glyphIndex < accepted.size(); glyphIndex++) {
                        final String record = glyphRecord(
                                page,
                                system.getId(),
                                glyphIndex,
                                glyphOrdinals.get(glyphIndex),
                                accepted.get(glyphIndex));
                        System.out.println(record);
                        systemRows.add(record);
                    }

                    System.out.printf(
                            "stemseedacceptsystemsummary %s system %d candidates %d checked %d "
                                    + "accepted %d %016x%n",
                            page,
                            system.getId(),
                            candidates.size(),
                            checked,
                            accepted.size(),
                            hash(systemRows));
                    sheetRows.addAll(systemRows);
                    sheetCandidates += candidates.size();
                    sheetChecked += checked;
                    sheetAccepted += accepted.size();
                }

                System.out.printf(
                        "stemseedacceptsheetsummary %s systems %d candidates %d checked %d "
                                + "accepted %d %016x%n",
                        page,
                        sheet.getSystems().size(),
                        sheetCandidates,
                        sheetChecked,
                        sheetAccepted,
                        hash(sheetRows));
                corpusSheets++;
                corpusSystems += sheet.getSystems().size();
                corpusCandidates += sheetCandidates;
                corpusChecked += sheetChecked;
                corpusAccepted += sheetAccepted;
                corpusRows.addAll(sheetRows);
            }
        }

        System.out.printf(
                "stemseedacceptcorpussummary sheets %d systems %d candidates %d checked %d "
                        + "accepted %d %016x%n",
                corpusSheets,
                corpusSystems,
                corpusCandidates,
                corpusChecked,
                corpusAccepted,
                hash(corpusRows));
        System.exit(0);
    }

    private static CheckerDetails inspectChecker (StemChecker checker,
                                                   StraightFilament candidate,
                                                   int profile)
        throws Exception
    {
        @SuppressWarnings("unchecked")
        final CheckSuite<Object> suite = (CheckSuite<Object>) GET_SUITE.invoke(checker, profile);
        final Object context = STICK_CONTEXT.newInstance(candidate);
        final List<Double> values = new ArrayList<>();

        for (Check<Object> check : suite.getChecks()) {
            Method getValue = null;
            for (Method method : check.getClass().getDeclaredMethods()) {
                if (method.getName().equals("getValue") && !method.isBridge()) {
                    getValue = method;
                    break;
                }
            }
            if (getValue == null) {
                throw new NoSuchMethodException(check.getClass().getName() + ".getValue");
            }
            getValue.setAccessible(true);
            values.add((Double) getValue.invoke(check, context));
        }

        final GradeImpacts impacts = checker.checkStem(candidate, profile);
        if (values.size() != impacts.getImpactCount()) {
            throw new IllegalStateException("raw checker value and impact counts differ");
        }

        return new CheckerDetails(
                suite,
                values,
                impacts,
                CONTEXT_GAP.getInt(context),
                CONTEXT_BLACK.getInt(context),
                CONTEXT_WHITE.getInt(context));
    }

    private static String glyphRecord (String page,
                                       int system,
                                       int glyphIndex,
                                       int ordinal,
                                       Glyph glyph)
    {
        final Rectangle box = glyph.getBounds();
        final Point2D start = glyph.getStartPoint(VERTICAL);
        final Point2D stop = glyph.getStopPoint(VERTICAL);
        final RunTable table = glyph.getRunTable();

        return String.format(
                "stemseedglyph %s system %d glyphIndex %d ordinal %d group VERTICAL_SEED "
                        + "free true bounds %d %d %d %d weight %d start %s %s stop %s %s "
                        + "thickness %s distance %s runs %d %016x",
                page,
                system,
                glyphIndex,
                ordinal,
                box.x,
                box.y,
                box.width,
                box.height,
                glyph.getWeight(),
                hex(start.getX()),
                hex(start.getY()),
                hex(stop.getX()),
                hex(stop.getY()),
                hex(glyph.getMeanThickness(VERTICAL)),
                hex(glyph.getMeanDistance()),
                runCount(table),
                runTableHash(table));
    }

    private static int runCount (RunTable table)
    {
        int count = 0;
        for (int sequence = 0; sequence < table.getSize(); sequence++) {
            for (java.util.Iterator<Run> it = table.iterator(sequence); it.hasNext();) {
                it.next();
                count++;
            }
        }
        return count;
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

    private static void printHeader ()
    {
        System.out.println(
                "# Java Audiveris 5.11 (Temurin JDK 25) accepted STEM_SEEDS glyphs.");
        System.out.println("# Generated from HEADERS plus StemScaler and the raw StickFactory");
        System.out.println("# boundary in stem-seeds.txt. The probe invokes the production private");
        System.out.println("# VerticalsBuilder.checkVerticals(Collection) method, then reads actual");
        System.out.println("# VERTICAL_SEED glyphs from each system's free-glyph collection.");
        System.out.println("#");
        System.out.println("# Checked rows expose exact raw SeedCheckSuite values, normalized impacts,");
        System.out.println("# weights, Clean side effects (gap/black/white), grade, and threshold.");
        System.out.println("# Skipped rows expose the production staff/tablature/header gate inputs.");
        System.out.println("# Glyph rows map actual materialized glyphs back to raw candidate ordinals;");
        System.out.println("# their run digest includes orientation, dimensions, and every run.");
        System.out.println("# All floating-point values are Double.toHexString exact encodings.");
        System.out.println("# Summary digests are FNV-1a-64 over exact record rows plus newline.");
        System.out.println("#");
        System.out.println("# Regenerate from the repository root:");
        System.out.println(
                "#   env -u JAVA_TOOL_OPTIONS JAVA_HOME=/path/to/jdk25/Contents/Home \\");
        System.out.println("#     ./gradlew --no-daemon -q \\");
        System.out.println(
                "#     -I rust/oracle/java/staff-impacts.init.gradle "
                        + ":app:stemSeedGlyphsProbe \\");
        System.out.println("#     '-PstemSeedTargets=data/examples/chula.png:1 ...'");
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

    private static class CandidateCheck
    {
        final int ordinal;

        final Point2D center;

        final Staff staff;

        final String reason;

        final CheckerDetails details;

        CandidateCheck (int ordinal,
                        Point2D center,
                        Staff staff,
                        String reason,
                        CheckerDetails details)
        {
            this.ordinal = ordinal;
            this.center = center;
            this.staff = staff;
            this.reason = reason;
            this.details = details;
        }

        String record (String page,
                       int system,
                       double threshold,
                       boolean accepted)
        {
            final int staffId = (staff != null) ? staff.getId() : -1;
            final int headerStop = (staff != null) ? staff.getHeaderStop() : -1;
            final boolean tablature = (staff != null) && staff.isTablature();
            final String prefix = String.format(
                    "%s %s system %d ordinal %d center %s %s staff %d headerStop %d "
                            + "tablature %s",
                    (details == null) ? "stemseedskip" : "stemseedcheck",
                    page,
                    system,
                    ordinal,
                    hex(center.getX()),
                    hex(center.getY()),
                    staffId,
                    headerStop,
                    tablature);

            if (details == null) {
                return prefix + " reason " + reason;
            }

            final StringBuilder values = new StringBuilder();
            final StringBuilder impacts = new StringBuilder();
            for (int index = 0; index < details.impacts.getImpactCount(); index++) {
                if (!values.isEmpty()) {
                    values.append(',');
                    impacts.append(',');
                }
                final String name = details.impacts.getName(index);
                values.append(name).append(':').append(hex(details.values.get(index)));
                impacts.append(name).append(':').append(hex(details.suite.getWeights().get(index)))
                        .append(':').append(hex(details.impacts.getImpact(index)));
            }

            return String.format(
                    "%s threshold %s values %s impacts %s gap %d black %d white %d grade %s "
                            + "accepted %s",
                    prefix,
                    hex(threshold),
                    values,
                    impacts,
                    details.gap,
                    details.black,
                    details.white,
                    hex(details.impacts.getGrade()),
                    accepted);
        }
    }

    private static class CheckerDetails
    {
        final CheckSuite<Object> suite;

        final List<Double> values;

        final GradeImpacts impacts;

        final int gap;

        final int black;

        final int white;

        CheckerDetails (CheckSuite<Object> suite,
                        List<Double> values,
                        GradeImpacts impacts,
                        int gap,
                        int black,
                        int white)
        {
            this.suite = suite;
            this.values = values;
            this.impacts = impacts;
            this.gap = gap;
            this.black = black;
            this.white = white;
        }
    }
}
