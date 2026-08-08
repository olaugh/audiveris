// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.Point;
import java.awt.Rectangle;
import java.awt.geom.Point2D;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.Iterator;
import java.util.List;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.Glyph;
import org.audiveris.omr.glyph.ShapeSet;
import org.audiveris.omr.image.Anchored.Anchor;
import org.audiveris.omr.image.Template;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.sheet.note.HeadSeedScale;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.sig.inter.Inters;
import org.audiveris.omr.sig.relation.HeadStemRelation;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.util.HorizontalSide;
import org.audiveris.omr.util.VerticalSide;

/**
 * Exact oracle for the first production semantic boundary of STEMS.
 *
 * <p>Each fresh JVM runs the requested page through the real HEADS step. The probe then reproduces
 * the head selection and stable abscissa sort at the start of
 * {@code StemsRetriever.inspectStems}. For each live stem-capable head it records the four
 * {@code HeadLinker.CLinker} constructor-order reference, outside and inside points. It stops
 * immediately before {@code CLinker.retrieveStump()}, so no STEMS glyph, attachment, linker, inter,
 * or relation mutation occurs.
 */
@SuppressWarnings("unchecked")
public class StemsHeadCornerProbe
{
    private StemsHeadCornerProbe ()
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
        cli.parseParameters("-batch", "-step", "HEADS");
        final java.lang.reflect.Field cliField = Main.class.getDeclaredField("cli");
        cliField.setAccessible(true);
        cliField.set(null, cli);
        org.audiveris.omr.ui.symbol.MusicFont.checkMusicFont();

        final String[] parts = args[0].split(":");
        if (parts.length != 2) {
            throw new IllegalArgumentException("target must be <path>:<sheet>");
        }
        runPage(Paths.get(parts[0]).toAbsolutePath(), Integer.parseInt(parts[1]));
        System.exit(0);
    }

    private static void runPage (Path path,
                                 int wanted)
        throws Exception
    {
        final Sheet sheet = loadPage(path, wanted);
        final String page = path.getFileName() + "#" + wanted;
        final HeadSeedScale headSeedScale = sheet.getScale().getHeadSeedScale();
        final RowHasher pageHeadHash = new RowHasher();
        final RowHasher pageCornerHash = new RowHasher();
        final RowHasher pageHash = new RowHasher();
        int pageHeads = 0;
        int pageCorners = 0;

        System.out.printf(
                "stemcornerpage %s systems %d staves %d family %s headSeedScale %s%n",
                page,
                sheet.getSystems().size(),
                sheet.getStaffManager().getStaffCount(),
                sheet.getStub().getMusicFamily(),
                headSeedScale != null);

        for (SystemInfo system : sheet.getSystems()) {
            final int profile = system.getProfile();
            final int maxHeadOutDx = sheet.getScale().toPixels(
                    HeadStemRelation.getXOutGapMaximum(profile));
            final int maxHeadInDx = sheet.getScale().toPixels(
                    HeadStemRelation.getXInGapMaximum(profile));
            final List<Inter> inSigOrder = system.getSig().inters(
                    ShapeSet.getTemplateNotesStem(sheet));
            final IdentityHashMap<HeadInter, Integer> sigOrdinals = new IdentityHashMap<>();
            final List<HeadInter> heads = new ArrayList<>(inSigOrder.size());
            for (int ordinal = 0; ordinal < inSigOrder.size(); ordinal++) {
                final HeadInter head = (HeadInter) inSigOrder.get(ordinal);
                if (head.isRemoved()) {
                    throw new IllegalStateException(
                            "STEMS selected removed head in system " + system.getId());
                }
                sigOrdinals.put(head, ordinal);
                heads.add(head);
            }
            Collections.sort(heads, (one, two) -> Inters.byAbscissa.compare(one, two));

            final RowHasher systemHeadHash = new RowHasher();
            final RowHasher systemCornerHash = new RowHasher();
            final RowHasher systemHash = new RowHasher();
            int systemCorners = 0;

            System.out.printf(
                    "stemcornersystem %s system %d profile %d interline %d maxHeadInDx %d "
                            + "maxHeadOutDx %d heads %d%n",
                    page,
                    system.getId(),
                    profile,
                    sheet.getScale().getInterline(),
                    maxHeadInDx,
                    maxHeadOutDx,
                    heads.size());

            for (int headOrdinal = 0; headOrdinal < heads.size(); headOrdinal++) {
                final HeadInter head = heads.get(headOrdinal);
                final Template template = head.getTemplate();
                if (template == null) {
                    throw new IllegalStateException("missing template for " + head);
                }
                if (template.getShape() != head.getShape()) {
                    throw new IllegalStateException("head/template shape mismatch for " + head);
                }
                if (template.getPointSize() != head.getStaff().getHeadPointSize()) {
                    throw new IllegalStateException("head/template point-size mismatch for " + head);
                }
                final Glyph glyph = head.getGlyph();
                if (glyph == null) {
                    throw new IllegalStateException("final HEADS head has no glyph: " + head);
                }
                final Rectangle box = head.getBounds();
                final Rectangle glyphBox = glyph.getBounds();
                final Rectangle slim = template.getSlimBounds();
                final String headRow = String.format(
                        "stemcornerhead %s system %d ordinal %d sigOrdinal %d staff %d shape %s "
                                + "bounds %s grade %s glyph bounds:%s:weight:%d:runs:%016x "
                                + "catalog %s:%d template %s:%s:%d:%d:%d:slim:%s",
                        page,
                        system.getId(),
                        headOrdinal,
                        sigOrdinals.get(head),
                        head.getStaff().getId(),
                        head.getShape(),
                        rectangle(box),
                        hexDouble(head.getGrade()),
                        rectangle(glyphBox),
                        glyph.getWeight(),
                        runTableHash(glyph),
                        template.getFamily(),
                        head.getStaff().getHeadPointSize(),
                        template.getShape(),
                        template.getFamily(),
                        template.getPointSize(),
                        template.getWidth(),
                        template.getHeight(),
                        rectangle(slim));
                addSemantic(headRow, systemHeadHash, systemHash, pageHeadHash, pageHash);
                System.out.println(headRow);

                final Double leftDx = (headSeedScale == null)
                        ? null : headSeedScale.getDx(head.getShape(), HorizontalSide.LEFT);
                final Double rightDx = (headSeedScale == null)
                        ? null : headSeedScale.getDx(head.getShape(), HorizontalSide.RIGHT);
                final Double appliedDx = appliedDx(leftDx, rightDx);
                int constructorOrdinal = 0;
                for (HorizontalSide horizontal : HorizontalSide.values()) {
                    final int xDir = horizontal.direction();
                    for (VerticalSide vertical : VerticalSide.values()) {
                        final Anchor anchor = anchor(horizontal, vertical);
                        final Rectangle templateBox = template.getBounds(box);
                        final Point offset = template.getOffset(anchor);
                        final Point2D uncorrected = new Point2D.Double(
                                templateBox.x + offset.x,
                                templateBox.y + offset.y);
                        final Point2D ref = head.getStemReferencePoint(horizontal, vertical);
                        final Point2D expected = expectedReference(
                                box, horizontal, uncorrected, appliedDx);
                        requireSamePoint(page, system, headOrdinal, anchor, expected, ref);
                        final Point2D out = new Point2D.Double(
                                ref.getX() + (xDir * maxHeadOutDx), ref.getY());
                        final Point2D in = new Point2D.Double(
                                ref.getX() - (xDir * maxHeadInDx), ref.getY());
                        final String cornerRow = String.format(
                                "stemcorner %s system %d head %d ctorOrdinal %d inspectOrdinal %d "
                                        + "corner %s anchor %s templateBounds %s offset %d:%d "
                                        + "seedDx %s:%s appliedDx %s uncorrected %s ref %s out %s in %s",
                                page,
                                system.getId(),
                                headOrdinal,
                                constructorOrdinal,
                                inspectionOrdinal(horizontal, vertical),
                                corner(horizontal, vertical),
                                anchor,
                                rectangle(templateBox),
                                offset.x,
                                offset.y,
                                nullableDouble(leftDx),
                                nullableDouble(rightDx),
                                nullableDouble(appliedDx),
                                point(uncorrected),
                                point(ref),
                                point(out),
                                point(in));
                        addSemantic(
                                cornerRow,
                                systemCornerHash,
                                systemHash,
                                pageCornerHash,
                                pageHash);
                        System.out.println(cornerRow);
                        constructorOrdinal++;
                        systemCorners++;
                    }
                }
                if (constructorOrdinal != 4) {
                    throw new IllegalStateException("head did not produce four constructor corners");
                }
            }

            if (systemCorners != 4 * heads.size()) {
                throw new IllegalStateException("system corner count mismatch");
            }
            System.out.printf(
                    "stemcornersystemsummary %s system %d heads %d corners %d headHash %016x "
                            + "cornerHash %016x hash %016x%n",
                    page,
                    system.getId(),
                    heads.size(),
                    systemCorners,
                    systemHeadHash.value(),
                    systemCornerHash.value(),
                    systemHash.value());
            pageHeads += heads.size();
            pageCorners += systemCorners;
        }

        System.out.printf(
                "stemcornerpagesummary %s systems %d heads %d corners %d headHash %016x "
                        + "cornerHash %016x hash %016x%n",
                page,
                sheet.getSystems().size(),
                pageHeads,
                pageCorners,
                pageHeadHash.value(),
                pageCornerHash.value(),
                pageHash.value());
    }

    private static Sheet loadPage (Path path,
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
        wantedStub.reachStep(OmrStep.HEADS, false);
        return wantedStub.getSheet();
    }

    private static Anchor anchor (HorizontalSide horizontal,
                                  VerticalSide vertical)
    {
        return switch (horizontal) {
            case LEFT -> switch (vertical) {
                case TOP -> Anchor.TOP_LEFT_STEM;
                case BOTTOM -> Anchor.BOTTOM_LEFT_STEM;
            };
            case RIGHT -> switch (vertical) {
                case TOP -> Anchor.TOP_RIGHT_STEM;
                case BOTTOM -> Anchor.BOTTOM_RIGHT_STEM;
            };
        };
    }

    private static String corner (HorizontalSide horizontal,
                                  VerticalSide vertical)
    {
        return "" + vertical.name().charAt(0) + horizontal.name().charAt(0);
    }

    private static int inspectionOrdinal (HorizontalSide horizontal,
                                          VerticalSide vertical)
    {
        if ((horizontal == HorizontalSide.RIGHT) && (vertical == VerticalSide.TOP)) {
            return 0;
        }
        if ((horizontal == HorizontalSide.LEFT) && (vertical == VerticalSide.BOTTOM)) {
            return 1;
        }
        if ((horizontal == HorizontalSide.LEFT) && (vertical == VerticalSide.TOP)) {
            return 2;
        }
        return 3;
    }

    private static Double appliedDx (Double left,
                                     Double right)
    {
        if (left != null) {
            return (right != null) ? (left + right) / 2 : left;
        }
        return right;
    }

    private static Point2D expectedReference (Rectangle box,
                                              HorizontalSide horizontal,
                                              Point2D uncorrected,
                                              Double appliedDx)
    {
        if (appliedDx == null) {
            return uncorrected;
        }
        final double x = (horizontal == HorizontalSide.LEFT)
                ? box.x + 0.5 - appliedDx
                : box.x + box.width - 1 + appliedDx;
        return new Point2D.Double(x, uncorrected.getY());
    }

    private static void requireSamePoint (String page,
                                          SystemInfo system,
                                          int headOrdinal,
                                          Anchor anchor,
                                          Point2D expected,
                                          Point2D actual)
    {
        if ((Double.doubleToLongBits(expected.getX()) != Double.doubleToLongBits(actual.getX()))
                || (Double.doubleToLongBits(expected.getY())
                        != Double.doubleToLongBits(actual.getY()))) {
            throw new IllegalStateException(String.format(
                    "%s system %d head %d anchor %s reference %s, expected %s",
                    page,
                    system.getId(),
                    headOrdinal,
                    anchor,
                    point(actual),
                    point(expected)));
        }
    }

    private static String rectangle (Rectangle box)
    {
        return String.format("%d:%d:%d:%d", box.x, box.y, box.width, box.height);
    }

    private static String point (Point2D point)
    {
        return hexDouble(point.getX()) + ":" + hexDouble(point.getY());
    }

    private static String nullableDouble (Double value)
    {
        return (value == null) ? "-" : hexDouble(value);
    }

    private static String hexDouble (double value)
    {
        return Double.toHexString(value) + "/"
                + String.format("%016x", Double.doubleToLongBits(value));
    }

    private static long runTableHash (Glyph glyph)
    {
        final List<String> rows = new ArrayList<>();
        final var table = glyph.getRunTable();
        rows.add(String.format(
                "%s %d %d", table.getOrientation(), table.getWidth(), table.getHeight()));
        for (int sequence = 0; sequence < table.getSize(); sequence++) {
            final StringBuilder row = new StringBuilder().append(sequence);
            for (Iterator<org.audiveris.omr.run.Run> it = table.iterator(sequence); it.hasNext();) {
                final var run = it.next();
                row.append(' ').append(run.getStart()).append(':').append(run.getLength());
            }
            rows.add(row.toString());
        }
        return hash(rows);
    }

    private static long hash (List<String> rows)
    {
        final RowHasher hash = new RowHasher();
        for (String row : rows) {
            hash.add(row);
        }
        return hash.value();
    }

    private static void addSemantic (String row,
                                     RowHasher... hashes)
    {
        for (RowHasher hash : hashes) {
            hash.add(row);
        }
    }

    private static void printHeader ()
    {
        System.out.println(
                "# Java Audiveris 5.11 (Temurin JDK 25.0.3+9 LTS) STEMS head-corner oracle.");
        System.out.println("#");
        System.out.println("# Each target runs real HEADS in a fresh JVM, selects the live stem-capable");
        System.out.println("# heads exactly as StemsRetriever.inspectStems, and stably sorts them only by");
        System.out.println("# bounds.x. sigOrdinal is the pre-sort live-head/SIG ordinal used for stable ties.");
        System.out.println("#");
        System.out.println("# Four corners per head follow HeadLinker constructor order: TL, BL, TR, BR.");
        System.out.println("# inspectOrdinal also records HeadCorner.values order: TR, BL, TL, BR.");
        System.out.println("# ref/out/in stop immediately before CLinker.retrieveStump; STEMS never mutates SIG,");
        System.out.println("# glyph index, attachments, inters, or relations in this probe.");
        System.out.println("#");
        System.out.println("# All identities are boundary-local ordinals. Doubles include hex text/raw bits.");
        System.out.println("# FNV-1a-64 hashes cover canonical UTF-8 semantic rows with trailing newlines.");
        System.out.println("# The runner appends a SHA-256 commitment over this header and emitted body.");
    }

    private static final class RowHasher
    {
        private long value = 0xcbf29ce484222325L;

        void add (String row)
        {
            for (byte value : (row + "\n").getBytes(StandardCharsets.UTF_8)) {
                this.value ^= Byte.toUnsignedLong(value);
                this.value *= 0x100000001b3L;
            }
        }

        long value ()
        {
            return value;
        }
    }
}
