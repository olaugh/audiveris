// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.awt.Point;
import java.awt.Rectangle;
import java.awt.geom.Point2D;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collection;
import java.util.EnumMap;
import java.util.EnumSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.Shape;
import org.audiveris.omr.glyph.ShapeSet;
import org.audiveris.omr.image.Anchored.Anchor;
import org.audiveris.omr.image.PixelDistance;
import org.audiveris.omr.image.Template;
import org.audiveris.omr.image.TemplateFactory;
import org.audiveris.omr.image.TemplateFactory.Catalog;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Scale;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.sheet.Staff;
import org.audiveris.omr.sheet.SystemInfo;
import org.audiveris.omr.step.OmrStep;
import org.audiveris.omr.sig.inter.HeadInter;
import org.audiveris.omr.sig.inter.Inter;
import org.audiveris.omr.ui.symbol.MusicFamily;
import org.audiveris.omr.ui.symbol.MusicFont;

/**
 * Exact oracle for the normal-staff HEADS template catalogs selected by production recognition.
 *
 * <p>Each page reaches the real HEADS step. This includes the BEAMS-side BlackHeadSizer and music
 * font sizing, LEDGERS, the HEADS distance-table prolog, and NoteHeadsBuilder itself. The probe
 * records every non-tablature staff's actual {@link Staff#getHeadPointSize()} and the catalog keyed
 * by that point size. Active shapes are obtained from the same three ShapeSet selectors used by
 * NoteHeadsBuilder. On the default example corpus this is the normal oval quartet, but the format
 * records and serializes any additional active head shape in {@link ShapeSet#Heads} factory order.
 *
 * <p>Every active template record includes exact geometry, every precise and rounded anchor, and
 * every PixelDistance in the row-major order emitted by TemplateFactory.Builder.buildKeyPoints.
 * Run one page per fresh JVM so process-global symbol and catalog caches cannot affect ordering.
 */
public class HeadTemplateCatalogProbe
{
    private HeadTemplateCatalogProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
    {
        if ((args.length == 1) && args[0].equals("--header")) {
            printHeader();
            System.exit(0);
        }

        final boolean smallHeads = (args.length == 2) && args[0].equals("--small-heads");
        if ((!smallHeads && (args.length != 1)) || (smallHeads && (args.length != 2))) {
            throw new IllegalArgumentException(
                    "expected [--small-heads] <path>:<sheet> target");
        }

        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        if (smallHeads) {
            cli.parseParameters(
                    "-batch",
                    "-step",
                    "HEADS",
                    "-constant",
                    "org.audiveris.omr.sheet.ProcessingSwitches.smallHeads=true");
        } else {
            cli.parseParameters("-batch", "-step", "HEADS");
        }
        final Field cliField = Main.class.getDeclaredField("cli");
        cliField.setAccessible(true);
        cliField.set(null, cli);
        MusicFont.checkMusicFont();

        final String[] parts = args[smallHeads ? 1 : 0].split(":");
        if (parts.length != 2) {
            throw new IllegalArgumentException("target must be <path>:<sheet>");
        }
        final Path path = Paths.get(parts[0]).toAbsolutePath();
        final int wanted = Integer.parseInt(parts[1]);
        runPage(path, wanted, smallHeads);
        System.exit(0);
    }

    private static void runPage (Path path,
                                 int wanted,
                                 boolean smallHeads)
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

        // This is intentionally the production step rather than a direct catalog construction.
        wantedStub.reachStep(OmrStep.HEADS, false);
        final Sheet sheet = wantedStub.getSheet();
        final String page = path.getFileName() + "#" + wanted;
        final MusicFamily family = sheet.getStub().getMusicFamily();
        final Scale scale = sheet.getScale();

        final EnumSet<Shape> activeAll = ShapeSet.getTemplateNotesAll(sheet);
        final EnumSet<Shape> activeStem = ShapeSet.getTemplateNotesStem(sheet);
        final EnumSet<Shape> activeHollow = ShapeSet.getTemplateNotesHollow(sheet);
        final EnumSet<Shape> active = EnumSet.copyOf(activeAll);
        active.addAll(activeStem);
        active.addAll(activeHollow);
        final EnumSet<Shape> normalOvals = EnumSet.of(
                Shape.NOTEHEAD_BLACK,
                Shape.NOTEHEAD_VOID,
                Shape.WHOLE_NOTE,
                Shape.BREVE);
        if (!active.containsAll(normalOvals)) {
            throw new IllegalStateException("normal oval templates are not all active");
        }

        final List<Shape> activeFactoryOrder = new ArrayList<>();
        for (Shape shape : ShapeSet.Heads) {
            if (active.contains(shape)) {
                activeFactoryOrder.add(shape);
            }
        }
        if (activeFactoryOrder.size() != active.size()) {
            throw new IllegalStateException("active template shape absent from ShapeSet.Heads");
        }

        System.out.printf(
                "headtemplatepage %s family %s systems %d staves %d mainInterline %d "
                        + "musicPointSize %d active %d%n",
                page,
                family,
                sheet.getSystems().size(),
                sheet.getStaffManager().getStaffCount(),
                scale.getInterline(),
                scale.getMusicFontScale().getPointSize(),
                activeFactoryOrder.size());

        final List<String> setRows = new ArrayList<>();
        setRows.add(printSet(page, "all", activeAll));
        setRows.add(printSet(page, "stem", activeStem));
        setRows.add(printSet(page, "hollow", activeHollow));
        System.out.printf("headtemplatesetssummary %s sets %d %016x%n",
                page, setRows.size(), hash(setRows));

        final Map<Integer, CatalogUsage> catalogs = new LinkedHashMap<>();
        final List<String> usageRows = new ArrayList<>();
        int staffCount = 0;
        int tablatureCount = 0;

        for (SystemInfo system : sheet.getSystems()) {
            for (Staff staff : system.getStaves()) {
                if (staff.isTablature()) {
                    tablatureCount++;
                    final String row = String.format(
                            "headtemplateusage %s system %d staff %d tablature true lines %d "
                                    + "interline %d catalog -",
                            page,
                            system.getId(),
                            staff.getId(),
                            staff.getLineCount(),
                            staff.getSpecificInterline());
                    usageRows.add(row);
                    System.out.println(row);
                    continue;
                }

                final int pointSize = staff.getHeadPointSize();
                CatalogUsage usage = catalogs.get(pointSize);
                if (usage == null) {
                    usage = new CatalogUsage(catalogs.size(), pointSize);
                    catalogs.put(pointSize, usage);
                }
                usage.staffIds.add(staff.getId());
                staffCount++;

                final String row = String.format(
                        "headtemplateusage %s system %d staff %d tablature false lines %d "
                                + "interline %d headPointSize %d catalog %d",
                        page,
                        system.getId(),
                        staff.getId(),
                        staff.getLineCount(),
                        staff.getSpecificInterline(),
                        pointSize,
                        usage.ordinal);
                usageRows.add(row);
                System.out.println(row);
            }
        }
        System.out.printf(
                "headtemplateusagesummary %s standard %d tablature %d catalogs %d %016x%n",
                page,
                staffCount,
                tablatureCount,
                catalogs.size(),
                hash(usageRows));

        final List<String> pageCatalogSummaries = new ArrayList<>();
        for (CatalogUsage usage : catalogs.values()) {
            final String summary = printCatalog(
                    page,
                    family,
                    usage,
                    activeFactoryOrder);
            pageCatalogSummaries.add(summary);
        }

        final List<String> pageRows = new ArrayList<>();
        pageRows.addAll(setRows);
        pageRows.addAll(usageRows);
        pageRows.addAll(pageCatalogSummaries);
        System.out.printf(
                "headtemplatepagesummary %s active %d standardStaves %d catalogs %d %016x%n",
                page,
                activeFactoryOrder.size(),
                staffCount,
                catalogs.size(),
                hash(pageRows));

        if (smallHeads) {
            printLiveHeadSummary(page, sheet);
        }
    }

    private static void printLiveHeadSummary (String page,
                                              Sheet sheet)
    {
        final Map<Shape, Integer> counts = new EnumMap<>(Shape.class);
        int total = 0;

        for (SystemInfo system : sheet.getSystems()) {
            for (Inter inter : system.getSig().inters(HeadInter.class)) {
                if (!inter.isRemoved()) {
                    counts.merge(inter.getShape(), 1, Integer::sum);
                    total++;
                }
            }
        }

        final List<String> parts = new ArrayList<>();
        for (Shape shape : ShapeSet.Heads) {
            final int count = counts.getOrDefault(shape, 0);
            if (count != 0) {
                parts.add(shape + ":" + count);
            }
        }
        System.out.printf(
                "headtemplateliveheads %s total %d shapes %s%n",
                page,
                total,
                String.join(",", parts));
    }

    private static String printCatalog (String page,
                                        MusicFamily family,
                                        CatalogUsage usage,
                                        List<Shape> activeShapes)
    {
        final Catalog catalog = TemplateFactory.getInstance().getCatalog(family, usage.pointSize);
        final String catalogRow = String.format(
                "headtemplatecatalog %s catalog %d family %s pointSize %d staves %s active %d",
                page,
                usage.ordinal,
                family,
                usage.pointSize,
                joinInts(usage.staffIds),
                activeShapes.size());
        System.out.println(catalogRow);

        final List<String> shapeSummaries = new ArrayList<>();
        for (int factoryOrdinal = 0; factoryOrdinal < ShapeSet.Heads.size(); factoryOrdinal++) {
            final Shape shape = ShapeSet.Heads.get(factoryOrdinal);
            final int activeOrdinal = activeShapes.indexOf(shape);
            if (activeOrdinal < 0) {
                continue;
            }
            shapeSummaries.add(printTemplate(
                    page,
                    usage.ordinal,
                    catalog,
                    shape,
                    factoryOrdinal,
                    activeOrdinal));
        }

        final List<String> rows = new ArrayList<>();
        rows.add(catalogRow);
        rows.addAll(shapeSummaries);
        final String summary = String.format(
                "headtemplatecatalogsummary %s catalog %d family %s pointSize %d shapes %d %016x",
                page,
                usage.ordinal,
                family,
                usage.pointSize,
                shapeSummaries.size(),
                hash(rows));
        System.out.println(summary);
        return summary;
    }

    private static String printTemplate (String page,
                                         int catalogOrdinal,
                                         Catalog catalog,
                                         Shape shape,
                                         int factoryOrdinal,
                                         int activeOrdinal)
    {
        final Template template = catalog.getTemplate(shape);
        if (template == null) {
            throw new IllegalStateException("missing template for active shape " + shape);
        }
        final Rectangle slim = template.getSlimBounds();
        final List<PixelDistance> pixels = template.getKeyPoints();
        final String templateRow = String.format(
                "headtemplate %s catalog %d factoryOrdinal %d activeOrdinal %d shape %s "
                        + "family %s pointSize %d width %d height %d slim %d %d %d %d "
                        + "anchors %d pixels %d",
                page,
                catalogOrdinal,
                factoryOrdinal,
                activeOrdinal,
                shape,
                template.getFamily(),
                template.getPointSize(),
                template.getWidth(),
                template.getHeight(),
                slim.x,
                slim.y,
                slim.width,
                slim.height,
                template.getOffsets().size(),
                pixels.size());
        System.out.println(templateRow);

        final List<String> rows = new ArrayList<>();
        rows.add(templateRow);
        int anchorOrdinal = 0;
        for (Anchor anchor : Anchor.values()) {
            final Point2D precise = template.getOffsets().get(anchor);
            if (precise == null) {
                continue;
            }
            final Point rounded = template.getOffset(anchor);
            final String row = String.format(
                    "headtemplateanchor %s catalog %d shape %s ordinal %d anchor %s "
                            + "dx %s dy %s dxBits %016x dyBits %016x rounded %d %d",
                    page,
                    catalogOrdinal,
                    shape,
                    anchorOrdinal,
                    anchor,
                    Double.toHexString(precise.getX()),
                    Double.toHexString(precise.getY()),
                    Double.doubleToLongBits(precise.getX()),
                    Double.doubleToLongBits(precise.getY()),
                    rounded.x,
                    rounded.y);
            rows.add(row);
            System.out.println(row);
            anchorOrdinal++;
        }

        for (int ordinal = 0; ordinal < pixels.size(); ordinal++) {
            final PixelDistance pixel = pixels.get(ordinal);
            if (pixel.d != Math.rint(pixel.d)) {
                throw new IllegalStateException(
                        "non-integral template distance for " + shape + " at " + ordinal);
            }
            final String row = String.format(
                    "headtemplatepixel %s catalog %d shape %s ordinal %d x %d y %d "
                            + "value %d distance %s bits %016x",
                    page,
                    catalogOrdinal,
                    shape,
                    ordinal,
                    pixel.x,
                    pixel.y,
                    (int) pixel.d,
                    Double.toHexString(pixel.d),
                    Double.doubleToLongBits(pixel.d));
            rows.add(row);
            System.out.println(row);
        }

        final String summary = String.format(
                "headtemplatesummary %s catalog %d shape %s anchors %d pixels %d %016x",
                page,
                catalogOrdinal,
                shape,
                anchorOrdinal,
                pixels.size(),
                hash(rows));
        System.out.println(summary);
        return summary;
    }

    private static String printSet (String page,
                                    String kind,
                                    Collection<Shape> shapes)
    {
        final String row = String.format(
                "headtemplateset %s kind %s count %d shapes %s",
                page,
                kind,
                shapes.size(),
                joinShapes(shapes));
        System.out.println(row);
        return row;
    }

    private static String joinShapes (Collection<Shape> shapes)
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

    private static String joinInts (Collection<Integer> values)
    {
        final StringBuilder sb = new StringBuilder();
        for (Integer value : values) {
            if (!sb.isEmpty()) {
                sb.append(',');
            }
            sb.append(value);
        }
        return sb.isEmpty() ? "-" : sb.toString();
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
        System.out.println("# Java Audiveris 5.11 (Temurin JDK 25) HEADS template-catalog oracle.");
        System.out.println("#");
        System.out.println("# Every page reaches the real HEADS step in a fresh JVM. Staff usage");
        System.out.println("# therefore records the actual BEAMS-derived getHeadPointSize result.");
        System.out.println("# Active shapes are the union of NoteHeadsBuilder's all/stem/hollow");
        System.out.println("# ShapeSet selectors, serialized in ShapeSet.Heads factory order.");
        System.out.println("#");
        System.out.println("# Anchors include exact Double.toHexString values, raw bits, and Java's");
        System.out.println("# asymmetric rounded Template.getOffset result. PixelDistance records");
        System.out.println("# retain factory row-major order and exact signed distance values/bits.");
        System.out.println("# Row digests are FNV-1a-64 over UTF-8 canonical records plus newline.");
        System.out.println("# Template.evaluate weights are foreground=0x1.8p2, background=0x1.0p0,");
        System.out.println("# hole=0x1.0p2; it compares only zero/nonzero after UNKNOWN filtering.");
        System.out.println("#");
        System.out.println("# Generate one target per fresh JVM, in this order: chula, allegretto,");
        System.out.println("# batuque, carmen, cucaracha, hove, zizi, BachInvention5.");
    }

    private static class CatalogUsage
    {
        final int ordinal;

        final int pointSize;

        final List<Integer> staffIds = new ArrayList<>();

        CatalogUsage (int ordinal,
                      int pointSize)
        {
            this.ordinal = ordinal;
            this.pointSize = pointSize;
        }
    }
}
