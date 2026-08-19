//------------------------------------------------------------------------------------------------//
// Exact oracle for explicitly requested Bravura HEADS template point sizes.
//
// The page-driven HeadTemplateCatalogProbe harvests the catalog sizes its
// corpus pages actually use (78..87 at ~300 DPI). Low-resolution scans need
// far smaller sizes (Schenker Universal Edition pages ask for ~27), and Java
// cannot always survive such scans end to end -- but the catalog for a
// (family, pointSize) pair is font-derived only, so it can be dumped for any
// requested size with the very same TemplateFactory the production step uses.
//
// Emits exactly the headtemplate/headtemplateanchor/headtemplatepixel record
// grammar of HeadTemplateCatalogProbe, restricted to the normal oval quartet
// in ShapeSet.Heads factory order (the only shapes the checked-in generator
// accepts). One fresh JVM per run keeps catalog caches inert.
//------------------------------------------------------------------------------------------------//
package org.audiveris.omr.rustport;

import java.awt.Point;
import java.awt.Rectangle;
import java.awt.geom.Point2D;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.List;

import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.glyph.Shape;
import org.audiveris.omr.glyph.ShapeSet;
import org.audiveris.omr.image.Anchored.Anchor;
import org.audiveris.omr.image.PixelDistance;
import org.audiveris.omr.image.Template;
import org.audiveris.omr.image.TemplateFactory;
import org.audiveris.omr.image.TemplateFactory.Catalog;
import org.audiveris.omr.ui.symbol.MusicFamily;
import org.audiveris.omr.ui.symbol.MusicFont;

public class HeadTemplatePointSizeProbe
{
    private HeadTemplatePointSizeProbe ()
    {
    }

    public static void main (String[] args)
        throws Exception
    {
        if (args.length != 1) {
            System.err.println("usage: HeadTemplatePointSizeProbe <size,size,...>");
            System.exit(2);
        }
        WellKnowns.ensureLoaded();
        MusicFont.populateAllSymbols();

        final MusicFamily family = MusicFamily.Bravura;
        final List<Shape> ovals = List.of(
                Shape.NOTEHEAD_BLACK,
                Shape.NOTEHEAD_VOID,
                Shape.WHOLE_NOTE,
                Shape.BREVE);
        for (int ordinal = 0; ordinal < ovals.size(); ordinal++) {
            if (ShapeSet.Heads.get(ordinal) != ovals.get(ordinal)) {
                throw new IllegalStateException(
                        "ShapeSet.Heads factory order no longer starts with the oval quartet");
            }
        }

        final String[] parts = args[0].split(",");
        int catalogOrdinal = 0;
        for (String part : parts) {
            final int pointSize = Integer.parseInt(part.trim());
            final String page = "explicit-size-" + pointSize + "#1";
            final Catalog catalog = TemplateFactory.getInstance().getCatalog(family, pointSize);
            for (int ordinal = 0; ordinal < ovals.size(); ordinal++) {
                printTemplate(page, catalogOrdinal, catalog, ovals.get(ordinal), ordinal);
            }
            catalogOrdinal++;
        }
        System.exit(0);
    }

    private static void printTemplate (String page,
                                       int catalogOrdinal,
                                       Catalog catalog,
                                       Shape shape,
                                       int ordinal)
    {
        final Template template = catalog.getTemplate(shape);
        if (template == null) {
            throw new IllegalStateException("missing template for " + shape);
        }
        final Rectangle slim = template.getSlimBounds();
        final List<PixelDistance> pixels = template.getKeyPoints();
        System.out.printf(
                "headtemplate %s catalog %d factoryOrdinal %d activeOrdinal %d shape %s "
                        + "family %s pointSize %d width %d height %d slim %d %d %d %d "
                        + "anchors %d pixels %d%n",
                page,
                catalogOrdinal,
                ordinal,
                ordinal,
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

        int anchorOrdinal = 0;
        for (Anchor anchor : Anchor.values()) {
            final Point2D precise = template.getOffsets().get(anchor);
            if (precise == null) {
                continue;
            }
            final Point rounded = template.getOffset(anchor);
            System.out.printf(
                    "headtemplateanchor %s catalog %d shape %s ordinal %d anchor %s "
                            + "dx %s dy %s dxBits %016x dyBits %016x rounded %d %d%n",
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
            anchorOrdinal++;
        }

        for (int pixelOrdinal = 0; pixelOrdinal < pixels.size(); pixelOrdinal++) {
            final PixelDistance pixel = pixels.get(pixelOrdinal);
            if (pixel.d != Math.rint(pixel.d)) {
                throw new IllegalStateException(
                        "non-integral template distance for " + shape + " at " + pixelOrdinal);
            }
            System.out.printf(
                    "headtemplatepixel %s catalog %d shape %s ordinal %d x %d y %d "
                            + "value %d distance %s bits %016x%n",
                    page,
                    catalogOrdinal,
                    shape,
                    pixelOrdinal,
                    pixel.x,
                    pixel.y,
                    (int) pixel.d,
                    Double.toHexString(pixel.d),
                    Double.doubleToLongBits(pixel.d));
        }
    }
}
