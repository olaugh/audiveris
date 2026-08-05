// SPDX-License-Identifier: AGPL-3.0-or-later

import java.awt.RenderingHints;
import java.awt.geom.AffineTransform;
import java.awt.image.BufferedImage;
import java.awt.image.Raster;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.util.ArrayList;
import java.util.List;
import org.apache.pdfbox.Loader;
import org.apache.pdfbox.cos.COSArray;
import org.apache.pdfbox.cos.COSBase;
import org.apache.pdfbox.cos.COSName;
import org.apache.pdfbox.io.RandomAccessReadBufferedFile;
import org.apache.pdfbox.pdmodel.PDDocument;
import org.apache.pdfbox.pdmodel.PDPage;
import org.apache.pdfbox.pdmodel.common.PDRectangle;
import org.apache.pdfbox.pdmodel.graphics.image.PDImage;
import org.apache.pdfbox.pdmodel.graphics.image.PDImageXObject;
import org.apache.pdfbox.rendering.ImageType;
import org.apache.pdfbox.rendering.PDFRenderer;
import org.apache.pdfbox.rendering.PageDrawer;
import org.apache.pdfbox.rendering.PageDrawerParameters;
import org.apache.pdfbox.util.Matrix;

/**
 * Pins what PDFBox does to a PDF page, at every level a native ingest has to
 * reproduce.
 *
 * <p>This generates {@code rust/oracle/pdf-pages.txt}. Audiveris renders PDFs
 * through {@code PDFRenderer.renderImageWithDPI(page, 300, ImageType.GRAY)}
 * under {@code ANTIALIAS_OFF} and {@code INTERPOLATION_BICUBIC}
 * ({@code ImageLoading.PdfboxLoader.getImage}), so that call is the authority
 * and this probe makes it verbatim.
 *
 * <p>Three record kinds, so that each layer of the port can be checked on its
 * own rather than only end to end:
 *
 * <dl>
 *   <dt>{@code image}</dt>
 *   <dd>One per image draw. Carries the XObject's declared geometry, its filter
 *       chain, an FNV-1a-64 of the <em>filtered stream bytes</em> -- what a Rust
 *       filter chain must produce -- and an FNV-1a-64 of the decoded raster,
 *       which additionally pins colour handling. The filter hash is what
 *       {@code CCITTFaxDecode} and {@code JBIG2Decode} are graded against.</dd>
 *   <dt>{@code draw}</dt>
 *   <dd>One per image draw, carrying the six-term {@link AffineTransform} that
 *       Java2D actually receives, at 17 significant digits. Composing this in
 *       closed form from the page box, the DPI and the content stream's
 *       {@code cm} gets the horizontal terms bit-identical and {@code m11}
 *       wrong in its last bits, which is worth hundreds of wrong samples, so
 *       the port is checked against PDFBox's own numbers.</dd>
 *   <dt>{@code page}</dt>
 *   <dd>One per page: the boxes, the rotation, the rendered size, and an
 *       FNV-1a-64 of the rendered raster. End to end.</dd>
 * </dl>
 *
 * <p>Note the rendered size is not derivable in {@code double}: PDFBox computes
 * the scale as {@code 300/72f}, so a 792 pt page is 3299.999874 and floors to
 * 3299 rather than 3300.
 *
 * <p>Run from the repository root. PDFBox is not a build dependency of the Rust
 * tree, so take the classpath from the Audiveris app:
 *
 * <pre>
 *   CP=$(./gradlew -q :app:printClasspath)
 *   javac -cp "$CP" -d /tmp/pdfprobe rust/oracle/java/PdfPageProbe.java
 *   java -cp "/tmp/pdfprobe:$CP" \
 *       -Dlogback.configurationFile=rust/oracle/java/logback-quiet.xml \
 *       PdfPageProbe scans/*.pdf &gt; rust/oracle/pdf-pages.txt
 * </pre>
 *
 * <p>The logging configuration matters: PDFBox logs to stdout by default, and
 * it has something to say about three of the seven corpus files. Those messages
 * are worth reading -- "Suspicious stream length 0" is PDFBox announcing that a
 * stream's {@code /Length} is a lie and it is scanning for {@code endstream}
 * instead, which the port has to do too -- so they go to stderr rather than
 * being silenced.
 */
public class PdfPageProbe {
    /** FNV-1a-64 offset basis. */
    private static final long BASIS = 0xcbf29ce484222325L;

    /** FNV-1a-64 prime. */
    private static final long PRIME = 0x100000001b3L;

    /** Records emitted for the page currently being rendered. */
    private static final List<String> pending = new ArrayList<>();

    public static void main(String[] args) throws Exception {
        for (String arg : args) {
            String name = new File(arg).getName();
            try (PDDocument doc = Loader.loadPDF(new RandomAccessReadBufferedFile(arg))) {
                RenderingHints hints = new RenderingHints(null);
                hints.put(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_OFF);
                hints.put(RenderingHints.KEY_INTERPOLATION,
                    RenderingHints.VALUE_INTERPOLATION_BICUBIC);
                PDFRenderer renderer = new ProbeRenderer(doc);
                renderer.setRenderingHints(hints);

                for (int index = 0; index < doc.getNumberOfPages(); index++) {
                    pending.clear();
                    BufferedImage out = renderer.renderImageWithDPI(index, 300, ImageType.GRAY);
                    for (String record : pending) {
                        System.out.println(name + " " + index + " " + record);
                    }
                    PDPage page = doc.getPage(index);
                    System.out.println(name + " " + index + " page"
                        + " media " + box(page.getMediaBox())
                        + " crop " + box(page.getCropBox())
                        + " rot " + page.getRotation()
                        + " render " + out.getWidth() + " " + out.getHeight()
                        + " type " + out.getType()
                        + " fnv " + String.format("%016x", hash(out.getRaster())));
                }
            }
        }
    }

    /** A renderer whose page drawer reports every image draw before making it. */
    private static final class ProbeRenderer extends PDFRenderer {
        ProbeRenderer(PDDocument document) {
            super(document);
        }

        @Override
        protected PageDrawer createPageDrawer(PageDrawerParameters parameters)
                throws IOException {
            return new ProbeDrawer(parameters);
        }
    }

    /**
     * Reports the composed transform and the image behind every {@code Do}.
     *
     * <p>The transform is composed exactly as {@code PageDrawer.drawImage} does
     * before handing it to {@code Graphics2D.drawImage}: the graphics device
     * transform, concatenated with the current transformation matrix, flipped
     * and normalised out of image space.
     */
    private static final class ProbeDrawer extends PageDrawer {
        private int ordinal;

        ProbeDrawer(PageDrawerParameters parameters) throws IOException {
            super(parameters);
        }

        @Override
        public void drawImage(PDImage image) throws IOException {
            int width = image.getWidth();
            int height = image.getHeight();

            AffineTransform composed =
                new AffineTransform(getGraphics().getTransform());
            Matrix ctm = getGraphicsState().getCurrentTransformationMatrix();
            AffineTransform placement = ctm.createAffineTransform();
            placement.scale(1.0 / width, -1.0 / height);
            placement.translate(0, -height);
            composed.concatenate(placement);
            double[] m = new double[6];
            composed.getMatrix(m);

            pending.add("image " + ordinal
                + " size " + width + " " + height
                + " bpc " + image.getBitsPerComponent()
                + " mask " + image.isStencil()
                + " cs " + colourSpace(image)
                + " filters " + filters(image)
                + " decode " + decodeArray(image)
                + " raw " + rawDigest(image)
                + " stream " + streamDigest(image)
                + " raster " + rasterDigest(image));
            pending.add(String.format(
                "draw %d m00 %.17g m10 %.17g m01 %.17g m11 %.17g m02 %.17g m12 %.17g",
                ordinal, m[0], m[1], m[2], m[3], m[4], m[5]));
            ordinal++;
            super.drawImage(image);
        }
    }

    /**
     * The image's bytes exactly as they sit in the file, hashed.
     *
     * <p>This is what a native parser must extract before any filter runs, and
     * it is the check that catches a wrong {@code /Length}: three corpus
     * sources declare {@code /Length 0} on streams that are not empty, and a
     * parser that believes them produces nothing here while still producing a
     * plausible-looking image from the scan fallback.
     */
    private static String rawDigest(PDImage image) {
        if (!(image instanceof PDImageXObject xobject)) {
            return "n/a n/a";
        }
        return digest(() -> xobject.getStream().getCOSObject().createRawInputStream());
    }

    /** The image's own bytes with its filter chain applied, hashed. */
    private static String streamDigest(PDImage image) {
        if (!(image instanceof PDImageXObject xobject)) {
            return "n/a n/a";
        }
        return digest(() -> xobject.getStream().createInputStream());
    }

    /** Opens a stream. */
    private interface StreamSource {
        InputStream open() throws IOException;
    }

    /** Length and FNV-1a-64 of everything `source` yields. */
    private static String digest(StreamSource source) {
        try (InputStream in = source.open()) {
            long hash = BASIS;
            long length = 0;
            byte[] buffer = new byte[1 << 16];
            for (int read = in.read(buffer); read > 0; read = in.read(buffer)) {
                for (int i = 0; i < read; i++) {
                    hash = (hash ^ (buffer[i] & 0xff)) * PRIME;
                }
                length += read;
            }
            return length + " " + String.format("%016x", hash);
        } catch (IOException | RuntimeException e) {
            return "error " + e.getClass().getSimpleName();
        }
    }

    /** The image decoded all the way to samples, hashed. */
    private static String rasterDigest(PDImage image) {
        try {
            BufferedImage decoded = image.getImage();
            if (decoded == null) {
                return "none n/a n/a n/a";
            }
            Raster raster = decoded.getRaster();
            return decoded.getWidth() + " " + decoded.getHeight() + " "
                + raster.getNumBands() + " " + String.format("%016x", hash(raster));
        } catch (IOException | RuntimeException e) {
            return "error n/a n/a " + e.getClass().getSimpleName();
        }
    }

    /** FNV-1a-64 over every sample, band-interleaved, row major. */
    private static long hash(Raster raster) {
        int width = raster.getWidth();
        int height = raster.getHeight();
        int bands = raster.getNumBands();
        int[] row = new int[width * bands];
        long hash = BASIS;
        for (int y = 0; y < height; y++) {
            raster.getPixels(raster.getMinX(), raster.getMinY() + y, width, 1, row);
            for (int i = 0; i < width * bands; i++) {
                hash = (hash ^ (row[i] & 0xff)) * PRIME;
            }
        }
        return hash;
    }

    private static String colourSpace(PDImage image) {
        try {
            return image.getColorSpace() == null ? "none" : image.getColorSpace().getName();
        } catch (IOException | RuntimeException e) {
            return "error";
        }
    }

    private static String filters(PDImage image) {
        COSBase filter = image.getCOSObject().getDictionaryObject(COSName.FILTER);
        if (filter instanceof COSName name) {
            return name.getName();
        }
        if (!(filter instanceof COSArray array)) {
            return "none";
        }
        List<String> names = new ArrayList<>();
        for (int i = 0; i < array.size(); i++) {
            COSBase entry = array.getObject(i);
            names.add(entry instanceof COSName name ? name.getName() : "?");
        }
        return names.isEmpty() ? "none" : String.join(",", names);
    }

    private static String decodeArray(PDImage image) {
        COSBase decode = image.getCOSObject().getDictionaryObject(COSName.DECODE);
        if (!(decode instanceof COSArray array)) {
            return "none";
        }
        StringBuilder text = new StringBuilder();
        for (int i = 0; i < array.size(); i++) {
            text.append(i == 0 ? "" : ",").append(array.getInt(i, -1));
        }
        return text.toString();
    }

    private static String box(PDRectangle rectangle) {
        return String.format("%.17g %.17g %.17g %.17g",
            rectangle.getLowerLeftX(), rectangle.getLowerLeftY(),
            rectangle.getUpperRightX(), rectangle.getUpperRightY());
    }
}
