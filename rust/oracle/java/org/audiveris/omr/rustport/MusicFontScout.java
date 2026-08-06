// SPDX-License-Identifier: AGPL-3.0-or-later
//
// A measurement, not a parity oracle: it answers how much of Java2D
// `ClefBuilder` actually depends on, before any font code is written.
//
// Two hypotheses went in. The first was that `centroidOffset` reads a *binary*
// coverage mask, since `ShapeSymbol.buildImage` sets KEY_ANTIALIASING to
// VALUE_ANTIALIAS_OFF. That is false, and this is what refuted it: the rendered
// clefs carry ~200 distinct alpha values. KEY_ANTIALIASING governs shape
// rendering, while the symbol is drawn through `TextLayout.draw`, which obeys
// KEY_TEXT_ANTIALIASING -- left at its platform default, and on. So the offset
// really is antialiasing coverage, as the handoff originally feared.
//
// The second hypothesis holds: `computeImage` renders at the fixed
// `SampleRepository.STANDARD_INTERLINE`, so the offset is a function of
// (family, shape) alone and never of the sheet. This asks for the same shape at
// seven interlines to check that the offsets come back bit-identical, which is
// what makes them pinnable as constants instead of demanding a rasteriser.
package org.audiveris.omr.rustport;

import org.audiveris.omr.glyph.Shape;
import org.audiveris.omr.ui.symbol.FontSymbol;
import org.audiveris.omr.ui.symbol.MusicFamily;
import org.audiveris.omr.ui.symbol.MusicFont;

import java.awt.Rectangle;
import java.awt.geom.Rectangle2D;
import java.awt.image.BufferedImage;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.Locale;
import java.util.TreeSet;

public class MusicFontScout
{
    private static final Shape[] HEADER_CLEF_SHAPES = {
        Shape.F_CLEF,
        Shape.G_CLEF,
        Shape.G_CLEF_8VA,
        Shape.G_CLEF_8VB,
        Shape.C_CLEF,
        Shape.PERCUSSION_CLEF
    };

    public static void main (String[] args)
            throws Exception
    {
        final MusicFamily family = MusicFamily.Bravura;

        // (a) Is the rendered alpha channel binary?
        //
        // `computeImage` is protected and the image field is private, so reach
        // both reflectively rather than re-deriving them here: the point of the
        // measurement is to read exactly what Java caches, not a lookalike.
        final Class<?> symbolClass = Class.forName("org.audiveris.omr.ui.symbol.ShapeSymbol");
        final Method computeImage = symbolClass.getDeclaredMethod("computeImage");
        computeImage.setAccessible(true);
        final Field imageField = symbolClass.getDeclaredField("image");
        imageField.setAccessible(true);
        final Field offsetField = symbolClass.getDeclaredField("centroidOffset");
        offsetField.setAccessible(true);

        for (Shape shape : HEADER_CLEF_SHAPES) {
            final FontSymbol fs = shape.getFontSymbolByInterline(family, 20);
            if (fs == null || fs.symbol == null) {
                System.out.printf("scout.symbol.%s=MISSING%n", shape);
                continue;
            }
            computeImage.invoke(fs.symbol);
            final BufferedImage image = (BufferedImage) imageField.get(fs.symbol);
            final TreeSet<Integer> alphas = new TreeSet<>();
            long covered = 0;
            for (int y = 0; y < image.getHeight(); y++) {
                for (int x = 0; x < image.getWidth(); x++) {
                    final int alpha = (image.getRGB(x, y) >>> 24) & 0xff;
                    alphas.add(alpha);
                    if (alpha > 0) {
                        covered++;
                    }
                }
            }
            System.out.printf(
                    Locale.ROOT,
                    "scout.alpha.%s=%dx%d covered=%d distinct=%s%n",
                    shape,
                    image.getWidth(),
                    image.getHeight(),
                    covered,
                    alphas);

            // (b) Does the offset depend on the interline it was asked for?
            //
            // Ask for the same shape at a spread of interlines and compare the
            // offsets. `computeImage` uses STANDARD_INTERLINE unconditionally,
            // so these should be identical to the last bit.
            final StringBuilder offsets = new StringBuilder();
            for (int interline : new int[] {10, 16, 20, 23, 27, 34, 48}) {
                final FontSymbol other = shape.getFontSymbolByInterline(family, interline);
                fs.symbol.getCentroid(new Rectangle(0, 0, 100, 100)); // force the offset
                other.symbol.getCentroid(new Rectangle(0, 0, 100, 100));
                final java.awt.geom.Point2D offset =
                        (java.awt.geom.Point2D) offsetField.get(other.symbol);
                if (offsets.length() > 0) {
                    offsets.append(' ');
                }
                offsets.append(String.format(
                        Locale.ROOT,
                        "%d:%.17f,%.17f",
                        interline,
                        offset.getX(),
                        offset.getY()));
            }
            System.out.printf("scout.offset.%s=%s%n", shape, offsets);
        }

        // (c) How much precision does the offset actually need?
        //
        // `ClefBuilder` uses it only as rint(box.centerX + box.width * offsetX),
        // and `box.width` is itself rint(layout width). So the tolerance is
        // 0.5/width: anything smaller rounds away entirely. Report the outline
        // box per interline alongside it, since that is the other font call.
        for (Shape shape : HEADER_CLEF_SHAPES) {
            for (int interline : new int[] {10, 16, 20, 23, 27, 34, 48}) {
                final FontSymbol fs = shape.getFontSymbolByInterline(family, interline);
                if (fs == null) {
                    continue;
                }
                final Rectangle2D box = fs.getLayout().getBounds();
                final int width = (int) Math.rint(box.getWidth());
                final int height = (int) Math.rint(box.getHeight());
                System.out.printf(
                        Locale.ROOT,
                        "scout.bounds.%s.%d=x=%.17f y=%.17f w=%.17f h=%.17f rint=%dx%d tolerance=%.9f%n",
                        shape,
                        interline,
                        box.getX(),
                        box.getY(),
                        box.getWidth(),
                        box.getHeight(),
                        width,
                        height,
                        width > 0 ? 0.5 / width : Double.NaN);
            }
        }

        System.out.printf("scout.pointsize.20=%d%n", MusicFont.getPointSize(20));
    }
}
