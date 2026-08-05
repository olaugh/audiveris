// SPDX-License-Identifier: AGPL-3.0-or-later

import java.awt.image.BufferedImage;
import java.io.File;
import javax.imageio.ImageIO;

/**
 * Prints what Java's ImageIO makes of each file named on the command line.
 *
 * <p>This generates {@code rust/oracle/jpeg-verdicts.txt}, which pins the other
 * half of JPEG parity: not what the samples are, but which files decode at all.
 * A port that accepts a file Audiveris rejects produces an image where Audiveris
 * produces an error, and no sample comparison can see that.
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
                verdict = "accept " + image.getWidth() + " " + image.getHeight() + " "
                    + image.getRaster().getNumBands();
            } catch (Throwable t) {
                String message = t.getMessage();
                verdict = "reject " + (message == null ? t.getClass().getSimpleName() : message);
            }
            System.out.println(arg + "\t" + verdict);
        }
    }
}
