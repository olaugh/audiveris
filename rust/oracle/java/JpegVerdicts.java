// SPDX-License-Identifier: AGPL-3.0-or-later

import java.awt.image.BufferedImage;
import java.io.File;
import javax.imageio.ImageIO;

/**
 * Prints what Java's ImageIO makes of each file named on the command line.
 *
 * <p>This generates {@code rust/oracle/jpeg-verdicts.txt}, which pins the two
 * halves of JPEG parity that libjpeg-turbo cannot pin on its own: which files
 * decode at all, and -- via an FNV-1a-64 of the raster -- what they decode to.
 * Audiveris reads JPEGs through this reader, so it is the authority; the
 * differential test against libjpeg-turbo is a faster, finer-grained proxy that
 * happens to disagree on some damaged input.
 *
 * <p>Run from the repository root, with paths relative to it:
 *
 * <pre>
 *   javac -d /tmp/jpegverdicts rust/oracle/java/JpegVerdicts.java
 *   java -cp /tmp/jpegverdicts JpegVerdicts \
 *       $(find rust/crates/audiveris-jpeg/tests/data -type f | sort) \
 *       data/examples/BachInvention5.jpg
 * </pre>
 */
public class JpegVerdicts {
    public static void main(String[] args) {
        for (String arg : args) {
            String verdict;
            try {
                BufferedImage image = ImageIO.read(new File(arg));
                int w = image.getWidth(), h = image.getHeight();
                int bands = image.getRaster().getNumBands();
                long hash = 0xcbf29ce484222325L;
                for (int y = 0; y < h; y++) {
                    for (int x = 0; x < w; x++) {
                        for (int b = 0; b < bands; b++) {
                            hash ^= image.getRaster().getSample(x, y, b) & 0xff;
                            hash *= 0x100000001b3L;
                        }
                    }
                }
                verdict = "accept " + w + " " + h + " " + bands + " "
                    + String.format("%016x", hash);
            } catch (Throwable t) {
                String message = t.getMessage();
                verdict = "reject " + (message == null ? t.getClass().getSimpleName() : message);
            }
            System.out.println(arg + "\t" + verdict);
        }
    }
}
