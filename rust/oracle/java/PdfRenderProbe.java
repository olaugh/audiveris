// SPDX-License-Identifier: AGPL-3.0-or-later

import java.awt.RenderingHints;
import java.awt.image.BufferedImage;
import java.io.File;
import org.apache.pdfbox.Loader;
import org.apache.pdfbox.cos.COSName;
import org.apache.pdfbox.io.RandomAccessReadBufferedFile;
import org.apache.pdfbox.pdmodel.PDDocument;
import org.apache.pdfbox.pdmodel.PDPage;
import org.apache.pdfbox.pdmodel.PDResources;
import org.apache.pdfbox.pdmodel.graphics.PDXObject;
import org.apache.pdfbox.pdmodel.graphics.image.PDImageXObject;
import org.apache.pdfbox.rendering.ImageType;
import org.apache.pdfbox.rendering.PDFRenderer;

/**
 * Reproduces Audiveris's PDF page render exactly, and reports what it produced.
 *
 * <p>This is the oracle for a native PDF ingest, and it is deliberately a copy
 * of {@code ImageLoading.PdfboxLoader.getImage}: antialiasing off, bicubic
 * interpolation, {@code ImageType.GRAY}, and the 300 DPI default of the
 * {@code pdfResolution} constant. Any of those changing in Audiveris must change
 * here too, or the oracle stops describing the target.
 *
 * <p>For each page it prints the source image geometry and filters, the rendered
 * geometry, an FNV-1a-64 of the rendered raster, and the gray histogram summary
 * that shows how much of the output is interpolated rather than copied.
 *
 * <p>Compile and run against the Audiveris runtime classpath:
 *
 * <pre>
 *   ./gradlew :app:compileJava
 *   CP=$(./gradlew -q :app:dumpClasspath)     # or assemble it however you like
 *   javac -cp "$CP" -d /tmp/pdfprobe rust/oracle/java/PdfRenderProbe.java
 *   java -cp "$CP:/tmp/pdfprobe" -Djava.awt.headless=true PdfRenderProbe page.pdf
 * </pre>
 */
public class PdfRenderProbe {
    /** Audiveris's `pdfResolution` default. */
    private static final int DPI = 300;

    public static void main(String[] args) throws Exception {
        for (String arg : args) {
            try (PDDocument doc = Loader.loadPDF(new RandomAccessReadBufferedFile(arg))) {
                System.out.printf("pdf %s pages %d%n", new File(arg).getName(),
                    doc.getNumberOfPages());

                // Audiveris's exact hints; both are load-bearing for the output.
                RenderingHints hints = new RenderingHints(null);
                hints.put(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_OFF);
                hints.put(RenderingHints.KEY_INTERPOLATION,
                    RenderingHints.VALUE_INTERPOLATION_BICUBIC);
                PDFRenderer renderer = new PDFRenderer(doc);
                renderer.setRenderingHints(hints);

                for (int index = 0; index < doc.getNumberOfPages(); index++) {
                    PDPage page = doc.getPage(index);
                    PDResources resources = page.getResources();
                    for (COSName name : resources.getXObjectNames()) {
                        PDXObject object = resources.getXObject(name);
                        if (object instanceof PDImageXObject image) {
                            System.out.printf("  source %d %s %dx%d bpc %d %s%n",
                                index + 1, name.getName(), image.getWidth(), image.getHeight(),
                                image.getCOSObject().getInt(COSName.BITS_PER_COMPONENT),
                                image.getCOSObject().getItem(COSName.FILTER));
                        }
                    }

                    BufferedImage out = renderer.renderImageWithDPI(index, DPI, ImageType.GRAY);
                    int width = out.getWidth();
                    int height = out.getHeight();
                    long hash = 0xcbf29ce484222325L;
                    int[] histogram = new int[256];
                    for (int y = 0; y < height; y++) {
                        for (int x = 0; x < width; x++) {
                            int sample = out.getRaster().getSample(x, y, 0) & 0xff;
                            histogram[sample]++;
                            hash ^= sample;
                            hash *= 0x100000001b3L;
                        }
                    }
                    int levels = 0;
                    for (int value = 0; value < 256; value++) {
                        if (histogram[value] > 0) {
                            levels++;
                        }
                    }
                    long extremes = (long) histogram[0] + histogram[255];
                    System.out.printf("  page %d %dx%d %016x levels %d interpolated %.2f%%%n",
                        index + 1, width, height, hash, levels,
                        100.0 * (((long) width * height) - extremes) / ((long) width * height));
                }
            }
        }
    }
}
