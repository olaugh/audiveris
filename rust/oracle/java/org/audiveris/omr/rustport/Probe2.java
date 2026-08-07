package org.audiveris.omr.rustport;

import org.audiveris.omr.glyph.Shape;
import org.audiveris.omr.ui.symbol.FontSymbol;
import org.audiveris.omr.ui.symbol.MusicFamily;

import java.awt.Font;
import java.awt.font.FontRenderContext;
import java.awt.font.GlyphVector;
import java.awt.font.TextLayout;
import java.awt.geom.Rectangle2D;

public class Probe2
{
    public static void main (String[] args) throws Exception
    {
        final MusicFamily family = MusicFamily.Bravura;
        int[][] cases = {{250, 0}};
        Shape[] shapes = {Shape.G_CLEF, Shape.C_CLEF, Shape.F_CLEF};
        for (int[] c : cases) {
            for (Shape shape : new Shape[]{Shape.G_CLEF, Shape.C_CLEF, Shape.F_CLEF}) {
                int il = c[0];
                FontSymbol fs = shape.getFontSymbolByInterline(family, il);
                Font font = fs.font;
                TextLayout layout = fs.getLayout();
                Rectangle2D lb = layout.getBounds();
                FontRenderContext frc = new FontRenderContext(null, true, true);
                String s = new String(Character.toChars(codeOf(shape)));
                GlyphVector gv = font.createGlyphVector(frc, s);
                Rectangle2D vb = gv.getVisualBounds();
                Rectangle2D ob = gv.getOutline().getBounds2D();
                Rectangle2D gob = gv.getGlyphOutline(0).getBounds2D();
                System.out.printf("%s il=%d size=%s%n", shape, il, font.getSize2D());
                System.out.printf("   layout.getBounds  y=%s h=%s%n", lb.getY(), lb.getHeight());
                System.out.printf("   gv.visualBounds   y=%s h=%s%n", vb.getY(), vb.getHeight());
                System.out.printf("   gv.outline.bounds y=%s h=%s%n", ob.getY(), ob.getHeight());
                System.out.printf("   glyphOutline      y=%s h=%s%n", gob.getY(), gob.getHeight());
                if (shape == Shape.G_CLEF) {
                    java.awt.geom.PathIterator it = gv.getGlyphOutline(0).getPathIterator(null);
                    double[] seg = new double[6];
                    double maxY = Double.NEGATIVE_INFINITY;
                    int n = 0;
                    while (!it.isDone()) {
                        int t = it.currentSegment(seg);
                        int k = (t == java.awt.geom.PathIterator.SEG_CUBICTO) ? 3
                              : (t == java.awt.geom.PathIterator.SEG_CLOSE) ? 0 : 1;
                        for (int i = 0; i < k; i++) {
                            if (-seg[2 * i + 1] > maxY) { maxY = -seg[2 * i + 1]; }
                        }
                        if (n < 6) { System.out.printf("   PATH t=%d %s %s %s %s %s %s%n", t, seg[0], seg[1], seg[2], seg[3], seg[4], seg[5]); }
                        n++;
                        it.next();
                    }
                    System.out.printf("   PATH segments=%d maxY(up)=%s%n", n, maxY);
                }
            }
        }
    }

    private static int codeOf (Shape shape)
    {
        return switch (shape) {
            case G_CLEF -> 0xE050;
            case C_CLEF -> 0xE05C;
            case F_CLEF -> 0xE062;
            default -> 0;
        };
    }
}
