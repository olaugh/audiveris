// SPDX-License-Identifier: AGPL-3.0-or-later
//
// The MusicFont oracle: what `ClefBuilder` needs from Java2D, captured as data.
//
// Two hypotheses went in. The first was that `centroidOffset` reads a *binary*
// coverage mask, since `ShapeSymbol.buildImage` sets KEY_ANTIALIASING to
// VALUE_ANTIALIAS_OFF. That is false, and this is what refuted it: the rendered
// clefs carry ~200 distinct alpha values. KEY_ANTIALIASING governs shape
// rendering, while the symbol is drawn through `TextLayout.draw`, which obeys
// KEY_TEXT_ANTIALIASING -- left at its platform default, and on. So the offset
// really is antialiasing coverage, and reproducing it exactly would mean
// reproducing Java2D's text rasteriser.
//
// The second hypothesis holds, and is what makes the first one moot:
// `computeImage` renders at the fixed `SampleRepository.STANDARD_INTERLINE`
// whatever font it was asked for, so the offset is a function of
// (family, shape) alone and never of the sheet. The `offset` rows below assert
// that by asking at seven interlines and emitting the value only once, having
// checked all seven agree to the bit.
//
// The `bounds` rows are the other font call, `TextLayout.getBounds()`. They are
// swept densely on purpose: the values look like an outline bbox in font units
// quantized to a 1/32 px grid, and a dense sweep is what lets the Rust side
// *test* that law rather than infer it from a few points.
package org.audiveris.omr.rustport;

import org.audiveris.omr.glyph.Shape;
import org.audiveris.omr.ui.symbol.FontSymbol;
import org.audiveris.omr.ui.symbol.MusicFamily;
import org.audiveris.omr.ui.symbol.MusicFont;

import java.awt.Rectangle;
import java.awt.geom.Point2D;
import java.awt.geom.Rectangle2D;
import java.awt.image.BufferedImage;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.Locale;
import java.util.TreeSet;

public class MusicFontScout
{
    /**
     * Every single-glyph shape the header stages need.
     *
     * <p>The clefs are `ClefBuilder.HEADER_CLEF_SHAPES`. The three alterations are what
     * `KeyBuilder` places, and `COMMON_TIME`/`CUT_TIME` are the two time signatures Bravura draws
     * with one codepoint.
     *
     * <p>Deliberately absent are the composite time shapes. `TIME_TWO_FOUR` and friends are
     * numerator-over-denominator symbols laid out from two `TextLayout`s, and `TIME_TWELVE` and
     * `TIME_SIXTEEN` are two codepoints in one string; neither is a single glyph, so neither is
     * measurable by the sweep below without first porting the composition.
     */
    private static final Shape[] HEADER_CLEF_SHAPES = {
        Shape.F_CLEF,
        Shape.G_CLEF,
        Shape.G_CLEF_8VA,
        Shape.G_CLEF_8VB,
        Shape.C_CLEF,
        Shape.PERCUSSION_CLEF,
        Shape.FLAT,
        Shape.NATURAL,
        Shape.SHARP,
        Shape.COMMON_TIME,
        Shape.CUT_TIME,
        // The time-signature digits the corpus uses (2/4, 3/4). Each is one codepoint
        // (TIME_ZERO + d); composite shapes such as TIME_TWO_FOUR are *drawn* by stacking two of
        // these via NumDenSymbol, so the digits' boxes are what the composite dimension is built
        // from.
        Shape.TIME_TWO,
        Shape.TIME_THREE,
        Shape.TIME_FOUR
    };

    /**
     * Interlines to check the offset's size-independence at. Deliberately spans
     * well past any real sheet: the claim is that the value does not move at
     * all, so extreme sizes are the cheapest way to falsify it.
     */
    private static final int[] OFFSET_INTERLINES = {10, 16, 20, 23, 27, 34, 48};

    /** Inclusive interline sweep for the outline bounds. */
    private static final int BOUNDS_MIN_INTERLINE = 5;

    private static final int BOUNDS_MAX_INTERLINE = 120;

    public static void main (String[] args)
            throws Exception
    {
        final MusicFamily family = MusicFamily.Bravura;

        // `computeImage` is protected and both fields are private, so reach them
        // reflectively rather than re-deriving them: the point is to read what
        // Java actually caches, not a lookalike that could agree by accident.
        final Class<?> symbolClass = Class.forName("org.audiveris.omr.ui.symbol.ShapeSymbol");
        final Method computeImage = symbolClass.getDeclaredMethod("computeImage");
        computeImage.setAccessible(true);
        final Field imageField = symbolClass.getDeclaredField("image");
        imageField.setAccessible(true);
        final Field offsetField = symbolClass.getDeclaredField("centroidOffset");
        offsetField.setAccessible(true);

        // Record the runtime. Everything below is produced by Java2D's font
        // machinery, so unlike the rest of the oracle these values are only as
        // portable as the JDK that made them -- and `manifest.sha256` pins a
        // different one than a casual local run will pick up.
        System.out.printf(
                "musicfont.runtime=%s %s%n",
                System.getProperty("java.vm.name"),
                System.getProperty("java.runtime.version"));
        System.out.printf("musicfont.family=%s%n", family);
        System.out.printf("musicfont.standard-interline=%d%n", 20);
        System.out.printf("musicfont.pointsize-per-interline=%d%n", MusicFont.getPointSize(1000) / 1000);
        System.out.printf(
                "musicfont.area-pitch-offset.FLAT=%s%n",
                Double.toString(
                        org.audiveris.omr.sig.inter.AbstractPitchedInter.getAreaPitchOffset(
                                org.audiveris.omr.glyph.Shape.FLAT)));
        System.out.printf(
                "musicfont.key-pitches.BASS-FLAT=%s%n",
                java.util.Arrays.toString(
                        org.audiveris.omr.sig.inter.KeyInter.getPitches(
                                org.audiveris.omr.sig.inter.ClefInter.ClefKind.BASS,
                                org.audiveris.omr.glyph.Shape.FLAT)));

        for (Shape shape : HEADER_CLEF_SHAPES) {
            final FontSymbol reference = shape.getFontSymbolByInterline(family, 20);

            if ((reference == null) || (reference.symbol == null)) {
                System.out.printf("musicfont.offset.%s=MISSING%n", shape);
                continue;
            }

            // Coverage census. This is the line that refuted "antialiasing is
            // off", so it stays in the oracle rather than being summarised away.
            computeImage.invoke(reference.symbol);
            final BufferedImage image = (BufferedImage) imageField.get(reference.symbol);
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
                    "musicfont.coverage.%s=%dx%d covered=%d distinct=%d opaque=%b%n",
                    shape,
                    image.getWidth(),
                    image.getHeight(),
                    covered,
                    alphas.size(),
                    alphas.size() <= 2);

            // Size-independence, asserted here rather than left for a reader to
            // eyeball across seven printed rows.
            Point2D agreed = null;

            for (int interline : OFFSET_INTERLINES) {
                final FontSymbol fs = shape.getFontSymbolByInterline(family, interline);
                fs.symbol.getCentroid(new Rectangle(0, 0, 100, 100)); // forces the offset
                final Point2D offset = (Point2D) offsetField.get(fs.symbol);

                if (agreed == null) {
                    agreed = offset;
                } else if ((offset.getX() != agreed.getX()) || (offset.getY() != agreed.getY())) {
                    throw new IllegalStateException(
                            "centroidOffset depends on interline for " + shape + ": " + agreed
                                    + " vs " + offset + " at " + interline);
                }
            }

            // `Double.toString`, not `%.17f`. These values get pinned as Rust
            // constants, so the text has to round-trip the exact bits, and 17
            // digits *after the point* is only 16 significant digits at this
            // magnitude -- one short of what a double can need. Java's
            // `Double.toString` emits the shortest string that reparses to the
            // same value, and Rust's `f64` parser honours the same guarantee.
            System.out.printf(
                    "musicfont.offset.%s=%s %s%n",
                    shape,
                    Double.toString(agreed.getX()),
                    Double.toString(agreed.getY()));
        }

        for (Shape shape : HEADER_CLEF_SHAPES) {
            for (int interline = BOUNDS_MIN_INTERLINE; interline <= BOUNDS_MAX_INTERLINE;
                    interline++) {
                final FontSymbol fs = shape.getFontSymbolByInterline(family, interline);

                if (fs == null) {
                    continue;
                }

                final Rectangle2D box = fs.getLayout().getBounds();
                System.out.printf(
                        "musicfont.bounds.%s.%d=%s %s %s %s%n",
                        shape,
                        interline,
                        Double.toString(box.getX()),
                        Double.toString(box.getY()),
                        Double.toString(box.getWidth()),
                        Double.toString(box.getHeight()));
            }
        }
    }
}
