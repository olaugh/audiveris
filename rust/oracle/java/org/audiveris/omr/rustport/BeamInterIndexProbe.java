package org.audiveris.omr.rustport;

import java.lang.reflect.Field;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.List;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sig.SIGraph;
import org.audiveris.omr.sig.inter.AbstractBeamInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.step.OmrStep;

/**
 * Frozen page-wide identity authority for live beams at the HEADS/STEMS seam.
 *
 * <p>The native SIG owns source, vertex, group, and runtime state. These rows
 * disclose only the three sheet-global facts that the legacy B14 verifier
 * still requires when a later SIDES transaction changes its base beam:
 * persistent Inter ID, sorted InterIndex ordinal, and VIP membership.
 */
public final class BeamInterIndexProbe
{
    private BeamInterIndexProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
    {
        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "GRID");
        final Field field = Main.class.getDeclaredField("cli");
        field.setAccessible(true);
        field.set(null, cli);
        org.audiveris.omr.ui.symbol.MusicFont.checkMusicFont();

        System.out.println("# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) beam InterIndex authority.");
        System.out.println("# schema: stems-beam-inter-index-v1");
        System.out.println("# InterIndex ordinals are the sorted-ID getEntities iteration order at HEADS.");

        for (String arg : args) {
            final String[] parts = arg.split(":");
            final Path path = Paths.get(parts[0]).toAbsolutePath();
            final int wanted = Integer.parseInt(parts[1]);
            final int wantedSystem = Integer.parseInt(parts[2]);
            final Book book = new Book(path);
            book.createStubs();

            for (SheetStub stub : book.getValidStubs()) {
                if (stub.getNumber() != wanted) {
                    continue;
                }
                stub.reachStep(OmrStep.HEADS, false);
                emit(path.getFileName() + "#" + wanted, stub.getSheet(), wantedSystem);
            }
        }
        System.exit(0);
    }

    private static void emit (String page,
                              Sheet sheet,
                              int wantedSystem)
        throws Exception
    {
        final IdentityHashMap<Inter, Integer> indexOrdinals = new IdentityHashMap<>();
        int indexOrdinal = 0;
        for (Inter inter : sheet.getInterIndex().getEntities()) {
            if ((inter == null) || (inter.getId() <= 0)
                    || (sheet.getInterIndex().getEntity(inter.getId()) != inter)) {
                throw new IllegalStateException("incomplete or ambiguous InterIndex");
            }
            indexOrdinals.put(inter, indexOrdinal++);
        }

        final List<String> rows = new ArrayList<>();
        int beams = 0;
        for (SystemInfo system : sheet.getSystems()) {
            if (system.getId() != wantedSystem) {
                continue;
            }
            final SIGraph sig = system.getSig();
            int vertexOrdinal = 0;
            int beamSigOrdinal = 0;
            for (Inter inter : sig.vertexSet()) {
                if (inter instanceof AbstractBeamInter) {
                    final Integer ordinal = indexOrdinals.get(inter);
                    if (ordinal == null) {
                        throw new IllegalStateException("live beam absent from InterIndex");
                    }
                    rows.add(String.format(
                            "stemsbeaminterindexentry %s system %d beamSig %d vertex %d interId %d indexOrdinal %d vip %s class %s",
                            page, system.getId(), beamSigOrdinal, vertexOrdinal, inter.getId(),
                            ordinal, inter.isVip(), inter.getClass().getSimpleName()));
                    beamSigOrdinal++;
                    beams++;
                }
                vertexOrdinal++;
            }
        }
        for (String row : rows) {
            System.out.println(row);
        }
        System.out.printf(
                "stemsbeaminterindexsummary %s system %d interIndexEntries %d beams %d rowsSha256 %s%n",
                page, wantedSystem, indexOrdinal, beams, sha256Rows(rows));
    }

    private static String sha256Rows (List<String> rows)
        throws Exception
    {
        final MessageDigest digest = MessageDigest.getInstance("SHA-256");
        for (String row : rows) {
            digest.update(row.getBytes(java.nio.charset.StandardCharsets.UTF_8));
            digest.update((byte) '\n');
        }
        final StringBuilder text = new StringBuilder();
        for (byte value : digest.digest()) {
            text.append(String.format("%02x", value & 0xff));
        }
        return text.toString();
    }
}
