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
import java.util.Arrays;
import java.util.Collection;
import java.util.Collections;
import java.util.Iterator;
import java.util.List;
import java.util.Map;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.Glyph;
import org.audiveris.omr.glyph.GlyphGroup;
import org.audiveris.omr.glyph.Glyphs;
import org.audiveris.omr.glyph.Shape;
import org.audiveris.omr.image.DistanceTable;
import org.audiveris.omr.image.TemplateFactory;
import org.audiveris.omr.run.Orientation;
import org.audiveris.omr.run.Run;
import org.audiveris.omr.run.RunTable;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Part;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.Staff;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.grid.LineInfo;
import org.audiveris.omr.sheet.note.DistancesBuilder;
import org.audiveris.omr.sheet.note.HeadSeedTally;
import org.audiveris.omr.sheet.note.HeadSpotsBuilder;
import org.audiveris.omr.sheet.note.NoteHeadsBuilder;
import org.audiveris.omr.sig.inter.LedgerInter;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.ui.symbol.MusicFamily;
import org.audiveris.omr.ui.symbol.MusicFont;
import org.audiveris.omr.util.HorizontalSide;

/**
 * Exact oracle for the immutable construction context of NoteHeadsBuilder.Scanner.
 *
 * <p>Each target reaches the real LEDGERS step, then runs the real HEADS prolog. The probe prepares
 * a NoteHeadsBuilder exactly through the immutable system fields at the start of buildHeads, but it
 * never invokes Scanner.lookup. It reflectively constructs seed-mode and range-mode scanners in
 * processStaff order and proves their static geometry agrees before recording it once. No head is
 * created, no SIG vertex is added, and no head/seed tally is mutated.
 *
 * <p>The boundary deliberately stops before competitor and frozen-bar slicing. It freezes the
 * schedule, scale parameters, catalog selection, offsets, staff/ledger source identities, farther
 * ledger axes, and complete theoretical-ordinate/range vectors. The vectors use run-length
 * encoding so every integer ordinate remains recoverable without a full-width record per pixel.
 */
public class HeadsScannerContextProbe
{
    private static final Class<?> BUILDER_CLASS = NoteHeadsBuilder.class;

    private static final Class<?> STAFF_LINE_ADAPTER_CLASS = nested("StaffLineAdapter");

    private static final Class<?> LEDGER_ADAPTER_CLASS = nested("LedgerAdapter");

    private static final Class<?> SCANNER_CLASS = nested("Scanner");

    private HeadsScannerContextProbe ()
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
        cli.parseParameters("-batch", "-step", "LEDGERS");
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

        wantedStub.reachStep(OmrStep.LEDGERS, false);
        final Sheet sheet = wantedStub.getSheet();
        final Scale scale = sheet.getScale();
        final String page = path.getFileName() + "#" + wanted;
        final MusicFamily family = sheet.getStub().getMusicFamily();

        // Exact HeadsStep.doProlog products, in production order.
        final DistanceTable distances = new DistancesBuilder(sheet).buildDistances();
        final Map<SystemInfo, List<Glyph>> sheetSpots = new HeadSpotsBuilder(sheet).getSpots();

        System.out.printf(
                "headscannerpage %s width %d height %d systems %d staves %d family %s%n",
                page,
                distances.getWidth(),
                distances.getHeight(),
                sheet.getSystems().size(),
                sheet.getStaffManager().getStaffCount(),
                family);

        final List<String> pageRows = new ArrayList<>();
        int pageStandardStaves = 0;
        int pageGeometries = 0;
        int pageSchedules = 0;

        for (SystemInfo system : sheet.getSystems()) {
            final List<Glyph> systemSpots = sheetSpots.get(system);
            if (systemSpots == null) {
                throw new IllegalStateException("HEADS prolog omitted system " + system.getId());
            }
            final NoteHeadsBuilder builder = new NoteHeadsBuilder(
                    system,
                    distances,
                    systemSpots,
                    new HeadSeedTally(),
                    false,
                    new java.util.TreeMap<>());
            prepareBuilder(builder, system, systemSpots);

            final String paramsRow = parameterRow(page, system, builder, scale);
            pageRows.add(paramsRow);
            System.out.println(paramsRow);

            int systemGeometries = 0;
            int systemSchedules = 0;
            final List<String> systemRows = new ArrayList<>();
            for (Staff staff : system.getStaves()) {
                final String staffRow = staffRow(page, system, staff, family);
                pageRows.add(staffRow);
                systemRows.add(staffRow);
                System.out.println(staffRow);
                if (staff.isTablature()) {
                    continue;
                }
                pageStandardStaves++;

                // Force the real family/point-size catalog lookup made by buildHeads.
                final int pointSize = staff.getHeadPointSize();
                if (TemplateFactory.getInstance().getCatalog(family, pointSize) == null) {
                    throw new IllegalStateException("missing catalog " + family + "/" + pointSize);
                }

                final List<Geometry> geometries = buildGeometries(builder, staff);
                final List<String> staffRows = new ArrayList<>();
                for (int ordinal = 0; ordinal < geometries.size(); ordinal++) {
                    final String row = geometryRow(page, system, staff, ordinal, geometries.get(ordinal));
                    pageRows.add(row);
                    systemRows.add(row);
                    staffRows.add(row);
                    System.out.println(row);
                }
                pageGeometries += geometries.size();
                systemGeometries += geometries.size();

                for (String phase : new String[] { "seed", "range" }) {
                    for (int ordinal = 0; ordinal < geometries.size(); ordinal++) {
                        final String row = String.format(
                                "headscannerschedule %s system %d staff %d phase %s ordinal %d "
                                        + "geometry %d",
                                page,
                                system.getId(),
                                staff.getId(),
                                phase,
                                ordinal,
                                ordinal);
                        pageRows.add(row);
                        systemRows.add(row);
                        staffRows.add(row);
                        System.out.println(row);
                        pageSchedules++;
                        systemSchedules++;
                    }
                }

                final String summary = String.format(
                        "headscannerstaffsummary %s system %d staff %d geometries %d schedules %d "
                                + "%016x",
                        page,
                        system.getId(),
                        staff.getId(),
                        geometries.size(),
                        2 * geometries.size(),
                        hash(staffRows));
                pageRows.add(summary);
                systemRows.add(summary);
                System.out.println(summary);
            }

            final String summary = String.format(
                    "headscannersystemsummary %s system %d staves %d geometries %d schedules %d "
                            + "%016x",
                    page,
                    system.getId(),
                    system.getStaves().size(),
                    systemGeometries,
                    systemSchedules,
                    hash(systemRows));
            pageRows.add(summary);
            System.out.println(summary);
        }

        System.out.printf(
                "headscannerpagesummary %s standardStaves %d geometries %d schedules %d %016x%n",
                page,
                pageStandardStaves,
                pageGeometries,
                pageSchedules,
                hash(pageRows));
    }

    private static void prepareBuilder (NoteHeadsBuilder builder,
                                        SystemInfo system,
                                        List<Glyph> systemSpots)
        throws Exception
    {
        setField(builder, "systemBarAreas", call(builder, "getSystemBarAreas"));
        setField(builder, "systemCompetitors", call(builder, "getSystemCompetitors"));

        final List<Glyph> systemSeeds = system.getGroupedGlyphs(GlyphGroup.VERTICAL_SEED);
        Collections.sort(systemSeeds, Glyphs.byOrdinate);
        Collections.sort(systemSpots, Glyphs.byOrdinate);
        setField(builder, "systemSeeds", systemSeeds);
    }

    private static String parameterRow (String page,
                                        SystemInfo system,
                                        NoteHeadsBuilder builder,
                                        Scale scale)
        throws Exception
    {
        final Object params = field(builder, "params");
        return String.format(
                "headscannerparams %s system %d mainInterline %d maxStem %d maxDistanceLow %s "
                        + "reallyBadDistance %s maxTemplateDx %d maxClosedDy %d maxOpenDy %d "
                        + "minBeamWidth %d vBarMargin %s minTemplateWidth %d templateHalf %d "
                        + "xOffsets %s",
                page,
                system.getId(),
                scale.getInterline(),
                scale.getMaxStem(),
                hexDouble(doubleField(params, "maxDistanceLow")),
                hexDouble(doubleField(params, "reallyBadDistance")),
                intField(params, "maxTemplateDx"),
                intField(params, "maxClosedDy"),
                intField(params, "maxOpenDy"),
                intField(params, "minBeamWidth"),
                hexDouble(doubleField(params, "vBarMargin")),
                intField(builder, "minTemplateWidth"),
                intField(builder, "templateHalf"),
                joinInts((int[]) field(builder, "xOffsets")));
    }

    private static String staffRow (String page,
                                    SystemInfo system,
                                    Staff staff,
                                    MusicFamily family)
    {
        final Part part = staff.getPart();
        final String partFields;
        if (part == null) {
            partFields = "part - merged false partFirst - partLast -";
        } else {
            partFields = String.format(
                    "part %d merged %s partFirst %d partLast %d",
                    part.getId(),
                    part.isMerged(),
                    part.getFirstStaff().getId(),
                    part.getLastStaff().getId());
        }
        return String.format(
                "headscannerstaff %s system %d staff %d tablature %s drum %s lines %d "
                        + "interline %d headerStop %d %s pointSize %d catalog %s/%d",
                page,
                system.getId(),
                staff.getId(),
                staff.isTablature(),
                staff.isDrum(),
                staff.getLineCount(),
                staff.getSpecificInterline(),
                staff.getHeaderStop(),
                partFields,
                staff.getHeadPointSize(),
                family,
                staff.getHeadPointSize());
    }

    private static List<Geometry> buildGeometries (NoteHeadsBuilder builder,
                                                   Staff staff)
        throws Exception
    {
        final List<Geometry> geometries = new ArrayList<>();
        final int lineCount = staff.getLineCount();
        final int maxPitch = lineCount;
        int pitch = -lineCount;
        Object previous = null;

        for (int lineIndex = 0; lineIndex < staff.getLines().size(); lineIndex++) {
            final LineInfo line = staff.getLines().get(lineIndex);
            final Object adapter = newStaffLineAdapter(builder, staff, line);
            geometries.add(newGeometry(
                    builder,
                    staff,
                    Source.staffLine(lineIndex, line),
                    adapter,
                    previous,
                    -1,
                    pitch++));
            geometries.add(newGeometry(
                    builder,
                    staff,
                    Source.staffLine(lineIndex, line),
                    adapter,
                    null,
                    0,
                    pitch++));
            if (pitch == maxPitch) {
                geometries.add(newGeometry(
                        builder,
                        staff,
                        Source.staffLine(lineIndex, line),
                        adapter,
                        null,
                        1,
                        pitch++));
            }
            previous = adapter;
        }

        if (lineCount == 1) {
            return geometries;
        }

        final Part part = staff.getPart();
        for (int dir : new int[] { -1, 1 }) {
            boolean lookFurther = true;
            if ((part != null) && part.isMerged()) {
                if ((dir > 0) && (staff == part.getFirstStaff())) {
                    lookFurther = false;
                } else if ((dir < 0) && (staff == part.getLastStaff())) {
                    lookFurther = false;
                }
            }

            pitch = dir * 4;
            for (int ledgerIndex = dir;; ledgerIndex += dir) {
                final List<LedgerInter> ledgers = staff.getLedgers(ledgerIndex);
                if ((ledgers == null) || ledgers.isEmpty()) {
                    break;
                }
                pitch += 2 * dir;
                char prefix = 'a';
                for (int ordinal = 0; ordinal < ledgers.size(); ordinal++) {
                    final Glyph glyph = ledgers.get(ordinal).getGlyph();
                    final Object adapter = newLedgerAdapter(
                            builder,
                            staff,
                            String.valueOf(prefix++),
                            glyph);
                    final Source source = Source.ledger(ledgerIndex, ordinal, glyph);
                    geometries.add(newGeometry(
                            builder,
                            staff,
                            source,
                            adapter,
                            null,
                            0,
                            pitch));
                    if (lookFurther) {
                        geometries.add(newGeometry(
                                builder,
                                staff,
                                source,
                                adapter,
                                null,
                                dir,
                                pitch + dir));
                    }
                }
            }
        }

        return geometries;
    }

    private static Geometry newGeometry (NoteHeadsBuilder builder,
                                         Staff staff,
                                         Source source,
                                         Object line,
                                         Object line2,
                                         int dir,
                                         int pitch)
        throws Exception
    {
        final Object seed = newScanner(builder, line, line2, dir, pitch, true);
        final Object range = newScanner(builder, line, line2, dir, pitch, false);
        final Geometry a = readGeometry(builder, staff, source, seed, line, line2);
        final Geometry b = readGeometry(builder, staff, source, range, line, line2);
        if (!a.equals(b)) {
            throw new IllegalStateException(
                    "seed/range scanner geometry differs for staff " + staff.getId() + " pitch "
                            + pitch);
        }
        return a;
    }

    @SuppressWarnings("unchecked")
    private static Geometry readGeometry (NoteHeadsBuilder builder,
                                          Staff staff,
                                          Source source,
                                          Object scanner,
                                          Object line,
                                          Object line2)
        throws Exception
    {
        final int left = (int) callDeclared(line.getClass(), line, "getLeftAbscissa");
        final int right = (int) callDeclared(line.getClass(), line, "getRightAbscissa");
        final int line2Left = (line2 == null) ? Integer.MIN_VALUE
                : (int) callDeclared(line2.getClass(), line2, "getLeftAbscissa");
        final int line2Right = (line2 == null) ? Integer.MIN_VALUE
                : (int) callDeclared(line2.getClass(), line2, "getRightAbscissa");
        final int rangeLeft = Math.max(left, staff.getHeaderStop());
        final int rangeRight = right - intField(builder, "minTemplateWidth");

        final List<Object> farther = (List<Object>) field(scanner, "ledgers");
        final List<String> fartherAxes = new ArrayList<>();
        for (Object adapter : farther) {
            final Point2D p1 = (Point2D) field(adapter, "left");
            final Point2D p2 = (Point2D) field(adapter, "right");
            fartherAxes.add(axis(p1, p2));
        }

        return new Geometry(
                source,
                intField(scanner, "interline"),
                intField(scanner, "dir"),
                intField(scanner, "pitch"),
                booleanField(scanner, "isOpen"),
                (int[]) field(scanner, "yOffsets"),
                shapeNames((Collection<Shape>) field(scanner, "scannerTemplateNotesAll")),
                shapeNames((Collection<Shape>) field(scanner, "scannerTemplateNotesStem")),
                shapeNames((Collection<Shape>) field(scanner, "scannerTemplateNotesHollow")),
                left,
                right,
                line2Left,
                line2Right,
                fartherAxes,
                ordinateRle(scanner, left, right),
                rangeLeft,
                rangeRight,
                ordinateRle(scanner, rangeLeft, rangeRight));
    }

    private static String geometryRow (String page,
                                       SystemInfo system,
                                       Staff staff,
                                       int ordinal,
                                       Geometry geometry)
    {
        return String.format(
                "headscannergeometry %s system %d staff %d ordinal %d source %s dir %d pitch %d "
                        + "open %s interline %d line %d %d line2 %s yOffsets %s all %s stem %s "
                        + "hollow %s farther %s ordinate %s range %d %d rangeOrdinate %s",
                page,
                system.getId(),
                staff.getId(),
                ordinal,
                geometry.source.text,
                geometry.dir,
                geometry.pitch,
                geometry.open,
                geometry.interline,
                geometry.left,
                geometry.right,
                (geometry.line2Left == Integer.MIN_VALUE) ? "-"
                        : geometry.line2Left + ":" + geometry.line2Right,
                joinInts(geometry.yOffsets),
                geometry.allShapes,
                geometry.stemShapes,
                geometry.hollowShapes,
                geometry.fartherAxes.isEmpty() ? "-" : String.join(",", geometry.fartherAxes),
                geometry.ordinateRle,
                geometry.rangeLeft,
                geometry.rangeRight,
                geometry.rangeOrdinateRle);
    }

    private static String ordinateRle (Object scanner,
                                       int left,
                                       int right)
        throws Exception
    {
        if (right < left) {
            return "-";
        }
        final Method method = SCANNER_CLASS.getDeclaredMethod("getTheoreticalOrdinate", int.class);
        method.setAccessible(true);
        final StringBuilder sb = new StringBuilder();
        int value = (int) method.invoke(scanner, left);
        int count = 1;
        for (int x = left + 1; x <= right; x++) {
            final int next = (int) method.invoke(scanner, x);
            if (next == value) {
                count++;
            } else {
                appendRun(sb, value, count);
                value = next;
                count = 1;
            }
        }
        appendRun(sb, value, count);
        return sb.toString();
    }

    private static void appendRun (StringBuilder sb,
                                   int value,
                                   int count)
    {
        if (!sb.isEmpty()) {
            sb.append(',');
        }
        sb.append(value).append(':').append(count);
    }

    private static Object newStaffLineAdapter (NoteHeadsBuilder builder,
                                               Staff staff,
                                               LineInfo line)
        throws Exception
    {
        return constructor(STAFF_LINE_ADAPTER_CLASS).newInstance(builder, staff, line);
    }

    private static Object newLedgerAdapter (NoteHeadsBuilder builder,
                                            Staff staff,
                                            String prefix,
                                            Glyph glyph)
        throws Exception
    {
        return constructor(LEDGER_ADAPTER_CLASS).newInstance(builder, staff, prefix, glyph);
    }

    private static Object newScanner (NoteHeadsBuilder builder,
                                      Object line,
                                      Object line2,
                                      int dir,
                                      int pitch,
                                      boolean useSeeds)
        throws Exception
    {
        return constructor(SCANNER_CLASS).newInstance(
                builder, line, line2, dir, pitch, useSeeds);
    }

    private static Constructor<?> constructor (Class<?> type)
    {
        final Constructor<?>[] constructors = type.getDeclaredConstructors();
        if (constructors.length != 1) {
            throw new IllegalStateException("expected one constructor for " + type.getName());
        }
        constructors[0].setAccessible(true);
        return constructors[0];
    }

    private static Class<?> nested (String simpleName)
    {
        try {
            return Class.forName(NoteHeadsBuilder.class.getName() + "$" + simpleName);
        } catch (ClassNotFoundException ex) {
            throw new ExceptionInInitializerError(ex);
        }
    }

    private static Object call (Object target,
                                String methodName)
        throws Exception
    {
        final Method method = BUILDER_CLASS.getDeclaredMethod(methodName);
        method.setAccessible(true);
        return method.invoke(target);
    }

    private static Object callDeclared (Class<?> type,
                                        Object target,
                                        String methodName)
        throws Exception
    {
        final Method method = type.getDeclaredMethod(methodName);
        method.setAccessible(true);
        return method.invoke(target);
    }

    private static Object field (Object target,
                                 String name)
        throws Exception
    {
        final Field field = target.getClass().getDeclaredField(name);
        field.setAccessible(true);
        return field.get(target);
    }

    private static void setField (Object target,
                                  String name,
                                  Object value)
        throws Exception
    {
        final Field field = BUILDER_CLASS.getDeclaredField(name);
        field.setAccessible(true);
        field.set(target, value);
    }

    private static int intField (Object target,
                                 String name)
        throws Exception
    {
        return ((Number) field(target, name)).intValue();
    }

    private static double doubleField (Object target,
                                       String name)
        throws Exception
    {
        return ((Number) field(target, name)).doubleValue();
    }

    private static boolean booleanField (Object target,
                                         String name)
        throws Exception
    {
        return (boolean) field(target, name);
    }

    private static String shapeNames (Collection<Shape> shapes)
    {
        final StringBuilder sb = new StringBuilder();
        for (Shape shape : shapes) {
            if (!sb.isEmpty()) {
                sb.append(',');
            }
            sb.append(shape);
        }
        return sb.isEmpty() ? "-" : sb.toString();
    }

    private static String joinInts (int[] values)
    {
        if (values.length == 0) {
            return "-";
        }
        final StringBuilder sb = new StringBuilder();
        for (int value : values) {
            if (!sb.isEmpty()) {
                sb.append(',');
            }
            sb.append(value);
        }
        return sb.toString();
    }

    private static String axis (Point2D left,
                                Point2D right)
    {
        return hexDouble(left.getX()) + ":" + hexDouble(left.getY()) + ":"
                + hexDouble(right.getX()) + ":" + hexDouble(right.getY());
    }

    private static String hexDouble (double value)
    {
        return Double.toHexString(value) + "/" + String.format("%016x", Double.doubleToLongBits(value));
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
        System.out.println("# Java Audiveris 5.11 (Temurin JDK 25) HEADS scanner-context oracle.");
        System.out.println("#");
        System.out.println("# Every target reaches real LEDGERS and runs real HEADS prolog in a fresh JVM.");
        System.out.println("# Private Scanner instances are constructed in processStaff order, but lookup");
        System.out.println("# is never called. Seed and range construction must have identical static geometry.");
        System.out.println("# Competitor/bar Area slicing is intentionally deferred to a later sub-gate.");
        System.out.println("#");
        System.out.println("# Doubles are Double.toHexString/raw-bits. Ordinate vectors are value:length RLE");
        System.out.println("# over every inclusive x in the declared interval. Row summaries are FNV-1a-64");
        System.out.println("# over UTF-8 canonical records with a trailing newline.");
        System.out.println("#");
        System.out.println("# Generate one target per fresh JVM: chula, allegretto, batuque, carmen,");
        System.out.println("# cucaracha, hove, zizi, BachInvention5.");
    }

    private static final class Source
    {
        final String text;

        private Source (String text)
        {
            this.text = text;
        }

        static Source staffLine (int index,
                                 LineInfo line)
        {
            final Point2D left = line.getEndPoint(HorizontalSide.LEFT);
            final Point2D right = line.getEndPoint(HorizontalSide.RIGHT);
            return new Source("staff-line:" + index + ":axis:" + axis(left, right));
        }

        static Source ledger (int index,
                              int ordinal,
                              Glyph glyph)
        {
            final Point2D left = glyph.getStartPoint(Orientation.HORIZONTAL);
            final Point2D right = glyph.getStopPoint(Orientation.HORIZONTAL);
            final Rectangle box = glyph.getBounds();
            return new Source(String.format(
                    "ledger:%d:%d:bounds:%d:%d:%d:%d:weight:%d:runs:%016x:axis:%s",
                    index,
                    ordinal,
                    box.x,
                    box.y,
                    box.width,
                    box.height,
                    glyph.getWeight(),
                    runTableHash(glyph.getRunTable()),
                    axis(left, right)));
        }

        @Override
        public boolean equals (Object obj)
        {
            return (obj instanceof Source other) && text.equals(other.text);
        }

        @Override
        public int hashCode ()
        {
            return text.hashCode();
        }
    }

    private static final class Geometry
    {
        final Source source;

        final int interline;

        final int dir;

        final int pitch;

        final boolean open;

        final int[] yOffsets;

        final String allShapes;

        final String stemShapes;

        final String hollowShapes;

        final int left;

        final int right;

        final int line2Left;

        final int line2Right;

        final List<String> fartherAxes;

        final String ordinateRle;

        final int rangeLeft;

        final int rangeRight;

        final String rangeOrdinateRle;

        Geometry (Source source,
                  int interline,
                  int dir,
                  int pitch,
                  boolean open,
                  int[] yOffsets,
                  String allShapes,
                  String stemShapes,
                  String hollowShapes,
                  int left,
                  int right,
                  int line2Left,
                  int line2Right,
                  List<String> fartherAxes,
                  String ordinateRle,
                  int rangeLeft,
                  int rangeRight,
                  String rangeOrdinateRle)
        {
            this.source = source;
            this.interline = interline;
            this.dir = dir;
            this.pitch = pitch;
            this.open = open;
            this.yOffsets = yOffsets;
            this.allShapes = allShapes;
            this.stemShapes = stemShapes;
            this.hollowShapes = hollowShapes;
            this.left = left;
            this.right = right;
            this.line2Left = line2Left;
            this.line2Right = line2Right;
            this.fartherAxes = fartherAxes;
            this.ordinateRle = ordinateRle;
            this.rangeLeft = rangeLeft;
            this.rangeRight = rangeRight;
            this.rangeOrdinateRle = rangeOrdinateRle;
        }

        @Override
        public boolean equals (Object obj)
        {
            if (!(obj instanceof Geometry other)) {
                return false;
            }
            return source.equals(other.source) && interline == other.interline && dir == other.dir
                    && pitch == other.pitch && open == other.open
                    && Arrays.equals(yOffsets, other.yOffsets) && allShapes.equals(other.allShapes)
                    && stemShapes.equals(other.stemShapes)
                    && hollowShapes.equals(other.hollowShapes) && left == other.left
                    && right == other.right && line2Left == other.line2Left
                    && line2Right == other.line2Right && fartherAxes.equals(other.fartherAxes)
                    && ordinateRle.equals(other.ordinateRle) && rangeLeft == other.rangeLeft
                    && rangeRight == other.rangeRight
                    && rangeOrdinateRle.equals(other.rangeOrdinateRle);
        }

        @Override
        public int hashCode ()
        {
            return source.hashCode();
        }
    }
}
