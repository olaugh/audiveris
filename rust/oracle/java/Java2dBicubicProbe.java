// SPDX-License-Identifier: AGPL-3.0-or-later

import java.awt.*;
import java.awt.geom.AffineTransform;
import java.awt.image.BufferedImage;

/**
 * Isolates the Java2D image transform that Audiveris's PDF render depends on.
 *
 * <p>Audiveris renders PDF pages through PDFBox with bicubic interpolation and
 * antialiasing off, so the port has to reproduce Java2D's transform exactly.
 * This probe takes PDFBox out of the picture: it draws a known bilevel pattern
 * through a known scale with the same two rendering hints, and prints an
 * FNV-1a-64 of the resulting gray raster for each geometry and scale.
 *
 * <p>Generates the expectations in {@code oracle/java2d-bicubic.txt}. Run it
 * with any JDK -- unlike the JPEG oracle this does not depend on which libjpeg
 * the JDK was built against, because no image codec is involved:
 *
 * <pre>
 *   javac -d /tmp/j2d rust/oracle/java/Java2dBicubicProbe.java
 *   java -cp /tmp/j2d -Djava.awt.headless=true Java2dBicubicProbe
 * </pre>
 */
public class Java2dBicubicProbe {
    static BufferedImage source(int w, int h, int type) {
        BufferedImage img = new BufferedImage(w, h, type);
        Graphics2D g = img.createGraphics();
        g.setColor(Color.WHITE);
        g.fillRect(0, 0, w, h);
        g.setColor(Color.BLACK);
        // A deterministic bilevel pattern with isolated pixels and runs.
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++)
                if (((x * 7 + y * 13) % 11) < 4) g.fillRect(x, y, 1, 1);
        g.dispose();
        return img;
    }

    static void run(String label, BufferedImage src, double sx, double sy) {
        int dw = (int) Math.ceil(src.getWidth() * sx);
        int dh = (int) Math.ceil(src.getHeight() * sy);
        BufferedImage dst = new BufferedImage(dw, dh, BufferedImage.TYPE_BYTE_GRAY);
        Graphics2D g = dst.createGraphics();
        g.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_OFF);
        g.setRenderingHint(RenderingHints.KEY_INTERPOLATION,
                RenderingHints.VALUE_INTERPOLATION_BICUBIC);
        g.drawImage(src, AffineTransform.getScaleInstance(sx, sy), null);
        g.dispose();
        long h = 0xcbf29ce484222325L;
        for (int y = 0; y < dh; y++)
            for (int x = 0; x < dw; x++) {
                h ^= dst.getRaster().getSample(x, y, 0) & 0xff;
                h *= 0x100000001b3L;
            }
        System.out.printf("CASE %s %dx%d->%dx%d %016x%n", label, src.getWidth(), src.getHeight(), dw, dh, h);
    }

    public static void main(String[] args) {
        for (int[] geom : new int[][]{{8, 8}, {12, 6}, {1, 1}, {2, 3}, {37, 41}, {64, 64}, {103, 57}}) {
            BufferedImage gray = source(geom[0], geom[1], BufferedImage.TYPE_BYTE_GRAY);
            BufferedImage binary = source(geom[0], geom[1], BufferedImage.TYPE_BYTE_BINARY);
            for (double s : new double[]{0.51, 1.07, 0.508, 0.660, 1.071, 0.25, 3.3, 1.0}) {
                run("g" + geom[0] + "x" + geom[1] + "s" + s, gray, s, s);
                run("b" + geom[0] + "x" + geom[1] + "s" + s, binary, s, s);
            }
        }
    }
}
