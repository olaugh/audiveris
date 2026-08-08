// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import ij.process.ByteProcessor;

import java.awt.Point;
import java.awt.Rectangle;
import java.awt.geom.Point2D;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collection;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.SortedMap;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.Glyph;
import org.audiveris.omr.glyph.GlyphFactory;
import org.audiveris.omr.glyph.GlyphGroup;
import org.audiveris.omr.image.ChamferDistance;
import org.audiveris.omr.image.DistanceTable;
import org.audiveris.omr.run.Orientation;
import org.audiveris.omr.run.Run;
import org.audiveris.omr.run.RunTable;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Picture;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.Staff;
import org.audiveris.omr.sheet.StaffLine;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.grid.LineInfo;
import org.audiveris.omr.sheet.note.DistancesBuilder;
import org.audiveris.omr.sheet.note.HeadSpotsBuilder;
import org.audiveris.omr.sig.inter.LedgerInter;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.HorizontalSide;

/**
 * Exact oracle for the sheet-level prolog of the production HEADS step.
 *
 * <p>The page first reaches the real LEDGERS step. The probe then calls
 * {@link DistancesBuilder#buildDistances()} and {@link HeadSpotsBuilder#getSpots()} in the same
 * order as {@code HeadsStep.doProlog}. It deliberately stops before {@code NoteHeadsBuilder}, so
 * the fixture isolates the bounded raster handoff from template matching and SIG mutation.
 *
 * <p>Run one page per fresh JVM. Audiveris has process-global registries, and a corpus produced in
 * one JVM would make order drift harder to distinguish from recognition drift.
 */
public class HeadsPrologProbe
{
    private HeadsPrologProbe ()
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
        final java.lang.reflect.Field cliField = Main.class.getDeclaredField("cli");
        cliField.setAccessible(true);
        cliField.set(null, cli);
        org.audiveris.omr.ui.symbol.MusicFont.checkMusicFont();

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
        final Picture picture = sheet.getPicture();
        final String page = path.getFileName() + "#" + wanted;
        final int width = picture.getWidth();
        final int height = picture.getHeight();

        System.out.printf("headsprologpage %s width %d height %d systems %d%n",
                page, width, height, sheet.getSystems().size());

        // These are exactly the three classes of foreground DistancesBuilder.paintLines paints.
        // Their canonical records are not dumped individually because the resulting output would
        // obscure the prolog product; exact counts and full-record digests still pin every input.
        final UpstreamState upstream = captureUpstream(sheet);
        System.out.printf(
                "headsprologstaffinput %s staves %d lineGlyphs %d %016x%n",
                page,
                upstream.staffCount,
                upstream.lineGlyphCount,
                hash(upstream.staffRows));
        System.out.printf(
                "headsprologledgerinput %s finalLedgers %d %016x%n",
                page,
                upstream.ledgerCount,
                hash(upstream.ledgerRows));
        System.out.printf(
                "headsprologseedinput %s acceptedSeeds %d %016x%n",
                page,
                upstream.seedCount,
                hash(upstream.seedRows));

        final RunTable headRuns = picture.getVerticalTable(Picture.TableKey.HEAD_SPOTS);
        if (headRuns == null) {
            throw new IllegalStateException("BEAMS did not retain HEAD_SPOTS");
        }
        System.out.printf(
                "headsprologheadspots %s orientation %s width %d height %d runs %d weight %d "
                        + "%s%n",
                page,
                headRuns.getOrientation(),
                headRuns.getWidth(),
                headRuns.getHeight(),
                headRuns.getTotalRunCount(),
                headRuns.getWeight(),
                pixelDigest(headRuns.getBuffer()));

        // Actual HeadsStep.doProlog order. buildDistances paints lines, ledgers and seeds white in
        // the BufferedImage it obtains from BINARY before running the transform. The resulting
        // black mask is exactly the set of cells still at VALUE_TARGET after TablePainter has
        // neutralized the same locations in the returned table.
        final DistanceTable distances = new DistancesBuilder(sheet).buildDistances();
        int blackCount = 0;
        long binaryHash = 0xcbf29ce484222325L;
        for (int i = 0, size = width * height; i < size; i++) {
            final int pixel;
            if (distances.getValue(i) == ChamferDistance.VALUE_TARGET) {
                blackCount++;
                pixel = 0;
            } else {
                pixel = 255;
            }
            binaryHash = (binaryHash ^ pixel) * 0x100000001b3L;
        }
        System.out.printf(
                "headsprologbinary %s width %d height %d black %d %016x%n",
                page,
                distances.getWidth(),
                distances.getHeight(),
                blackCount,
                binaryHash);

        int unknownCount = 0;
        for (int i = 0, size = distances.getWidth() * distances.getHeight(); i < size; i++) {
            if (distances.getValue(i) == ChamferDistance.VALUE_UNKNOWN) {
                unknownCount++;
            }
        }
        System.out.printf(
                "headsprologdistances %s width %d height %d normalizer %d unknown %d %016x%n",
                page,
                distances.getWidth(),
                distances.getHeight(),
                distances.getNormalizer(),
                unknownCount,
                signedI32Hash(distances));

        final Map<SystemInfo, List<Glyph>> dispatched = new HeadSpotsBuilder(sheet).getSpots();

        // Build a second component list only to recover the global ordinals that getSpots keeps
        // internally. This calls the same GlyphFactory on the same immutable table. Full geometry
        // and runs, rather than a digest alone, are used to map the two lists.
        final List<Glyph> components = GlyphFactory.buildGlyphs(headRuns, new Point(0, 0));
        final Map<String, Integer> ordinalByKey = new HashMap<>();
        final List<String> componentRows = new ArrayList<>();

        for (int ordinal = 0; ordinal < components.size(); ordinal++) {
            final Glyph glyph = components.get(ordinal);
            final String key = glyphKey(glyph);
            if (ordinalByKey.put(key, ordinal) != null) {
                throw new IllegalStateException("duplicate exact HEAD_SPOT component key");
            }
            final Rectangle box = glyph.getBounds();
            final Point centroid = glyph.getCentroid();
            final RunTable table = glyph.getRunTable();
            final String row = String.format(
                    "headsprologcomponent %s ordinal %d bounds %d %d %d %d weight %d "
                            + "centroid %d %d runs %d %016x",
                    page,
                    ordinal,
                    box.x,
                    box.y,
                    box.width,
                    box.height,
                    glyph.getWeight(),
                    centroid.x,
                    centroid.y,
                    table.getTotalRunCount(),
                    runTableHash(table));
            componentRows.add(row);
            System.out.println(row);
        }
        System.out.printf(
                "headsprologcomponentsummary %s components %d %016x%n",
                page,
                componentRows.size(),
                hash(componentRows));

        int dispatchedReferences = 0;
        final Set<Integer> dispatchedOrdinals = new HashSet<>();
        final List<String> dispatchRows = new ArrayList<>();
        for (SystemInfo system : sheet.getSystems()) {
            final List<Glyph> spots = dispatched.get(system);
            if (spots == null) {
                throw new IllegalStateException("getSpots omitted system " + system.getId());
            }
            final StringBuilder ordinals = new StringBuilder();
            for (Glyph spot : spots) {
                final Integer ordinal = ordinalByKey.get(glyphKey(spot));
                if (ordinal == null) {
                    throw new IllegalStateException("dispatched HEAD_SPOT is not a component");
                }
                if (!ordinals.isEmpty()) {
                    ordinals.append(',');
                }
                ordinals.append(ordinal);
                dispatchedOrdinals.add(ordinal);
                dispatchedReferences++;
            }
            if (ordinals.isEmpty()) {
                ordinals.append('-');
            }
            final String row = String.format(
                    "headsprologdispatch %s system %d count %d ordinals %s",
                    page,
                    system.getId(),
                    spots.size(),
                    ordinals);
            dispatchRows.add(row);
            System.out.println(row);
        }
        System.out.printf(
                "headsprologdispatchsummary %s systems %d references %d uniqueComponents %d "
                        + "%016x%n",
                page,
                dispatchRows.size(),
                dispatchedReferences,
                dispatchedOrdinals.size(),
                hash(dispatchRows));
        System.out.printf(
                "headsprologsummary %s staves %d lineGlyphs %d finalLedgers %d acceptedSeeds %d "
                        + "components %d systems %d%n",
                page,
                upstream.staffCount,
                upstream.lineGlyphCount,
                upstream.ledgerCount,
                upstream.seedCount,
                componentRows.size(),
                dispatchRows.size());
    }

    private static UpstreamState captureUpstream (Sheet sheet)
    {
        final UpstreamState state = new UpstreamState();

        for (SystemInfo system : sheet.getSystems()) {
            int staffOrdinal = 0;
            for (Staff staff : system.getStaves()) {
                state.staffCount++;
                state.staffRows.add(String.format(
                        "staff system %d ordinal %d id %d tablature %s lines %d",
                        system.getId(),
                        staffOrdinal,
                        staff.getId(),
                        staff.isTablature(),
                        staff.getLines().size()));

                int lineOrdinal = 0;
                for (LineInfo line : staff.getLines()) {
                    final Glyph glyph = ((StaffLine) line).getGlyph();
                    if (glyph == null) {
                        throw new IllegalStateException("persistent staff line has no glyph");
                    }
                    final Rectangle box = glyph.getBounds();
                    final Point2D left = line.getEndPoint(HorizontalSide.LEFT);
                    final Point2D right = line.getEndPoint(HorizontalSide.RIGHT);
                    state.lineGlyphCount++;
                    state.staffRows.add(String.format(
                            "line system %d staff %d ordinal %d bounds %d %d %d %d weight %d "
                                    + "left %s %s right %s %s thickness %s runs %d %016x paint "
                                    + "%016x",
                            system.getId(),
                            staff.getId(),
                            lineOrdinal,
                            box.x,
                            box.y,
                            box.width,
                            box.height,
                            glyph.getWeight(),
                            hex(left.getX()),
                            hex(left.getY()),
                            hex(right.getX()),
                            hex(right.getY()),
                            hex(glyph.getMeanThickness(Orientation.HORIZONTAL)),
                            glyph.getRunTable().getTotalRunCount(),
                            runTableHash(glyph.getRunTable()),
                            linePaintHash(line, glyph)));
                    lineOrdinal++;
                }

                final SortedMap<Integer, List<LedgerInter>> ledgerMap = staff.getLedgerMap();
                for (Map.Entry<Integer, List<LedgerInter>> entry : ledgerMap.entrySet()) {
                    int ledgerOrdinal = 0;
                    for (LedgerInter ledger : entry.getValue()) {
                        final Glyph glyph = ledger.getGlyph();
                        if (glyph == null) {
                            throw new IllegalStateException("final ledger has no glyph");
                        }
                        state.ledgerCount++;
                        state.ledgerRows.add(glyphInputRecord(
                                "ledger",
                                system.getId(),
                                staff.getId(),
                                entry.getKey(),
                                ledgerOrdinal,
                                glyph));
                        ledgerOrdinal++;
                    }
                }
                staffOrdinal++;
            }

            int seedOrdinal = 0;
            for (Glyph seed : system.getGroupedGlyphs(GlyphGroup.VERTICAL_SEED)) {
                state.seedCount++;
                state.seedRows.add(glyphInputRecord(
                        "seed", system.getId(), -1, 0, seedOrdinal, seed));
                seedOrdinal++;
            }
        }

        return state;
    }

    private static String glyphInputRecord (String kind,
                                            int system,
                                            int staff,
                                            int index,
                                            int ordinal,
                                            Glyph glyph)
    {
        final Rectangle box = glyph.getBounds();
        final RunTable table = glyph.getRunTable();
        return String.format(
                "%s system %d staff %d index %d ordinal %d bounds %d %d %d %d weight %d "
                        + "runs %d %016x",
                kind,
                system,
                staff,
                index,
                ordinal,
                box.x,
                box.y,
                box.width,
                box.height,
                glyph.getWeight(),
                table.getTotalRunCount(),
                runTableHash(table));
    }

    private static long linePaintHash (LineInfo line,
                                       Glyph glyph)
    {
        final double halfLine = 0.5 * glyph.getMeanThickness(Orientation.HORIZONTAL);
        final Point2D left = line.getEndPoint(HorizontalSide.LEFT);
        final Point2D right = line.getEndPoint(HorizontalSide.RIGHT);
        final int xMin = (int) Math.floor(left.getX());
        final int xMax = (int) Math.ceil(right.getX());
        long hash = 0xcbf29ce484222325L;

        for (int x = xMin; x <= xMax; x++) {
            final double ordinate = line.yAt((double) x);
            final int yMin = (int) Math.rint(ordinate - halfLine);
            final int yMax = (int) Math.rint(ordinate + halfLine);
            hash = hashSignedI32(hash, x);
            hash = hashSignedI32(hash, yMin);
            hash = hashSignedI32(hash, yMax);
        }
        return hash;
    }

    private static String glyphKey (Glyph glyph)
    {
        final Rectangle box = glyph.getBounds();
        final Point centroid = glyph.getCentroid();
        final RunTable table = glyph.getRunTable();
        final StringBuilder key = new StringBuilder();
        key.append(box.x).append(' ').append(box.y).append(' ').append(box.width).append(' ')
                .append(box.height).append(' ').append(glyph.getWeight()).append(' ')
                .append(centroid.x).append(' ').append(centroid.y).append(' ')
                .append(table.getOrientation()).append(' ').append(table.getWidth()).append(' ')
                .append(table.getHeight());
        for (int sequence = 0; sequence < table.getSize(); sequence++) {
            key.append('|').append(sequence);
            for (Iterator<Run> it = table.iterator(sequence); it.hasNext();) {
                final Run run = it.next();
                key.append(' ').append(run.getStart()).append(':').append(run.getLength());
            }
        }
        return key.toString();
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

    private static String pixelDigest (ByteProcessor buffer)
    {
        long hash = 0xcbf29ce484222325L;
        for (int i = 0, size = buffer.getWidth() * buffer.getHeight(); i < size; i++) {
            hash = (hash ^ (buffer.get(i) & 0xff)) * 0x100000001b3L;
        }
        return String.format("%016x", hash);
    }

    private static long signedI32Hash (DistanceTable table)
    {
        long hash = 0xcbf29ce484222325L;
        for (int i = 0, size = table.getWidth() * table.getHeight(); i < size; i++) {
            hash = hashSignedI32(hash, table.getValue(i));
        }
        return hash;
    }

    private static long hashSignedI32 (long hash,
                                       int value)
    {
        for (int shift = 0; shift < Integer.SIZE; shift += Byte.SIZE) {
            hash = (hash ^ ((value >>> shift) & 0xffL)) * 0x100000001b3L;
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

    private static String hex (double value)
    {
        return Double.toHexString(value);
    }

    private static void printHeader ()
    {
        System.out.println("# Java Audiveris 5.11 (Temurin JDK 25) HEADS prolog oracle.");
        System.out.println("#");
        System.out.println("# Every page reaches the real LEDGERS step in a fresh JVM. The probe then");
        System.out.println("# calls DistancesBuilder.buildDistances() and HeadSpotsBuilder.getSpots()");
        System.out.println("# in HeadsStep.doProlog source order, stopping before NoteHeadsBuilder.");
        System.out.println("#");
        System.out.println("# Staff/ledger/seed input digests cover exact paint-order records, glyph");
        System.out.println("# bounds and complete run tables. Staff records additionally cover every");
        System.out.println("# synthetic line-paint x/y range used by DistancesBuilder.paintLines.");
        System.out.println("# Pixel digests are FNV-1a-64 in row-major byte order. Distance digests");
        System.out.println("# are FNV-1a-64 over row-major signed i32 values in little-endian order.");
        System.out.println("# Record digests include a trailing newline. Doubles use toHexString.");
        System.out.println("# Components retain GlyphFactory's unsorted order. Dispatch records name");
        System.out.println("# those ordinals exactly; one component may belong to several systems.");
        System.out.println("#");
        System.out.println("# Compile and run from app/ with the frozen JDK and saved runtime classpath.");
        System.out.println("# Generate one target per fresh JVM, in this order: chula, allegretto,");
        System.out.println("# batuque, carmen, cucaracha, hove, zizi, BachInvention5.");
    }

    private static class UpstreamState
    {
        int staffCount;

        int lineGlyphCount;

        int ledgerCount;

        int seedCount;

        final List<String> staffRows = new ArrayList<>();

        final List<String> ledgerRows = new ArrayList<>();

        final List<String> seedRows = new ArrayList<>();
    }
}
