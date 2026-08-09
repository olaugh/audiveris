// SPDX-License-Identifier: AGPL-3.0-or-later
package org.audiveris.omr.rustport;

import java.io.InputStream;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.MessageDigest;
import java.util.HexFormat;
import java.util.List;
import java.util.Locale;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

import javax.xml.parsers.DocumentBuilderFactory;

import org.audiveris.omr.CLI;
import org.audiveris.omr.Main;
import org.audiveris.omr.WellKnowns;
import org.audiveris.omr.sheet.Book;
import org.audiveris.omr.sheet.Sheet;
import org.audiveris.omr.sheet.SheetStub;
import org.audiveris.omr.step.OmrStep;

import org.w3c.dom.Document;
import org.w3c.dom.Element;
import org.w3c.dom.NodeList;

/**
 * Runs one Java sheet through {@link OmrStep#PAGE} and exports it to one explicit MusicXML path.
 *
 * <p>This probe is intentionally separate from {@link SigProbe}. Recognition streaming uses the
 * {@code @omrscope } prefix and frames stage snapshots; export has one terminal record with the
 * distinct {@code @omrscope-export } prefix. Existing oracle and stream readers therefore never
 * have to learn an export event.
 *
 * <p>Arguments are {@code <input> <one-based-sheet> <output.mxl|output.xml>}. The output must not
 * exist. A successful record is emitted only after the artifact has been opened as MusicXML,
 * measured, and hashed. {@link Sheet#export(Path)} logs and swallows its own write errors, so this
 * validation is part of the correctness boundary rather than an optional diagnostic.
 */
public final class MusicXmlExportProbe
{
    private static final String PREFIX = "@omrscope-export ";

    private static final long MAX_XML_ENTRY_BYTES = 256L * 1024L * 1024L;

    private MusicXmlExportProbe ()
    {
    }

    public static void main (String[] args)
    {
        final long startedAt = System.nanoTime();
        int sheetNumber = 0;
        Path output = null;
        boolean createdOutput = false;

        try {
            if (args.length != 3) {
                throw failure(
                        "invalid_arguments",
                        "expected <input> <one-based-sheet> <output.mxl|output.xml>");
            }

            final Path input = Paths.get(args[0]).toAbsolutePath().normalize();
            try {
                sheetNumber = Integer.parseInt(args[1]);
            } catch (NumberFormatException ex) {
                throw failure("invalid_arguments", "sheet must be a positive integer", ex);
            }
            output = Paths.get(args[2]).toAbsolutePath().normalize();

            if (sheetNumber <= 0) {
                throw failure("invalid_arguments", "sheet must be a positive integer");
            }
            if (!Files.isRegularFile(input)) {
                throw failure("input_missing", "input is not a regular file: " + input);
            }
            final String format = outputFormat(output);
            if (Files.exists(output, LinkOption.NOFOLLOW_LINKS)) {
                throw failure("output_exists", "output already exists: " + output);
            }

            configureBatchCli();
            org.audiveris.omr.ui.symbol.MusicFont.checkMusicFont();

            final Book book = new Book(input);
            book.createStubs();
            final SheetStub stub = selectSheet(book.getValidStubs(), sheetNumber, input);

            try {
                stub.reachStep(OmrStep.PAGE, false);
            } catch (Exception ex) {
                throw failure("page_failed", rootMessage(ex), ex);
            }
            if (!stub.isDone(OmrStep.PAGE)) {
                throw failure("page_failed", "PAGE did not reach a completed state");
            }

            final Sheet sheet = stub.getSheet();
            if (sheet == null) {
                throw failure("page_failed", "PAGE completed without a sheet");
            }
            if (sheet.getPages().size() != 1) {
                // Sheet.export writes sibling .mvtN files for this case. Refuse it here so the
                // completion path remains exact instead of relying on a discovery glob.
                throw failure(
                        "unsupported_multipage",
                        "sheet contains " + sheet.getPages().size()
                                + " score pages; exact single-artifact export requires one");
            }

            sheet.export(output);
            createdOutput = Files.exists(output, LinkOption.NOFOLLOW_LINKS);
            validateMusicXml(output, format);

            final Path canonical = output.toRealPath();
            final long bytes = Files.size(canonical);
            final String sha256 = sha256(canonical);
            markerSuccess(
                    sheetNumber,
                    format,
                    canonical,
                    bytes,
                    sha256,
                    System.nanoTime() - startedAt);
            System.exit(0);
        } catch (Throwable ex) {
            final ProbeFailure failure = asProbeFailure(ex);
            markerFailure(
                    sheetNumber,
                    failure.code,
                    failure.getMessage(),
                    System.nanoTime() - startedAt);

            // The path was required not to exist before this attempt. If this probe created a
            // partial artifact and then rejected it, removing that artifact cannot delete caller
            // data and prevents a later viewer run from mistaking it for a valid export.
            if (createdOutput && (output != null)) {
                try {
                    Files.deleteIfExists(output);
                } catch (Exception ignored) {
                    // The terminal failure remains authoritative; cleanup is best-effort only.
                }
            }
            System.exit(2);
        }
    }

    private static void configureBatchCli ()
        throws Exception
    {
        // The step engine consults Main.getCli() while PAGE is running. Direct Book users must
        // install the same batch CLI state that Main.main would install.
        final CLI cli = new CLI(WellKnowns.TOOL_NAME);
        cli.parseParameters("-batch", "-step", "PAGE");
        final Field field = Main.class.getDeclaredField("cli");
        field.setAccessible(true);
        field.set(null, cli);
    }

    private static SheetStub selectSheet (List<SheetStub> stubs,
                                          int wanted,
                                          Path input)
        throws ProbeFailure
    {
        for (SheetStub stub : stubs) {
            if (stub.getNumber() == wanted) {
                return stub;
            }
        }
        throw failure("sheet_missing", "no valid sheet " + wanted + " in " + input);
    }

    private static String outputFormat (Path output)
        throws ProbeFailure
    {
        final String name = output.getFileName().toString().toLowerCase(Locale.ROOT);
        if (name.endsWith(".mxl")) {
            return "mxl";
        }
        if (name.endsWith(".xml")) {
            return "musicxml";
        }
        throw failure("invalid_arguments", "output extension must be .mxl or .xml");
    }

    private static void validateMusicXml (Path output,
                                          String format)
        throws ProbeFailure
    {
        try {
            if (!Files.isRegularFile(output) || (Files.size(output) <= 0)) {
                throw failure("export_missing", "export did not create a non-empty regular file");
            }

            if ("mxl".equals(format)) {
                validateMxl(output);
            } else {
                try (InputStream input = Files.newInputStream(output)) {
                    validateScoreDocument(input);
                }
            }
        } catch (ProbeFailure ex) {
            throw ex;
        } catch (Exception ex) {
            throw failure("export_invalid", rootMessage(ex), ex);
        }
    }

    private static void validateMxl (Path output)
        throws Exception
    {
        try (ZipFile zip = new ZipFile(output.toFile())) {
            final ZipEntry containerEntry = zip.getEntry("META-INF/container.xml");
            if ((containerEntry == null) || containerEntry.isDirectory()) {
                throw failure("export_invalid", "MXL has no META-INF/container.xml");
            }

            final Document container;
            try (InputStream input = zip.getInputStream(containerEntry)) {
                container = parseXml(input);
            }
            final NodeList rootFiles = container.getElementsByTagNameNS("*", "rootfile");
            if (rootFiles.getLength() != 1) {
                throw failure(
                        "export_invalid",
                        "MXL container must declare exactly one rootfile, found "
                                + rootFiles.getLength());
            }
            final String rootPath = ((Element) rootFiles.item(0)).getAttribute("full-path");
            if (rootPath.isBlank() || rootPath.startsWith("/") || rootPath.contains("..")) {
                throw failure("export_invalid", "MXL container has an unsafe rootfile path");
            }
            final ZipEntry scoreEntry = zip.getEntry(rootPath);
            if ((scoreEntry == null) || scoreEntry.isDirectory()) {
                throw failure("export_invalid", "MXL rootfile is absent: " + rootPath);
            }
            if ((scoreEntry.getSize() < 1) || (scoreEntry.getSize() > MAX_XML_ENTRY_BYTES)) {
                throw failure("export_invalid", "MXL rootfile has an invalid uncompressed size");
            }
            try (InputStream input = zip.getInputStream(scoreEntry)) {
                validateScoreDocument(input);
            }
        }
    }

    private static void validateScoreDocument (InputStream input)
        throws Exception
    {
        final Document document = parseXml(input);
        final Element root = document.getDocumentElement();
        final String name = (root.getLocalName() != null) ? root.getLocalName() : root.getNodeName();
        if (!"score-partwise".equals(name) && !"score-timewise".equals(name)) {
            throw failure("export_invalid", "MusicXML root is " + name);
        }
    }

    private static Document parseXml (InputStream input)
        throws Exception
    {
        final DocumentBuilderFactory factory = DocumentBuilderFactory.newInstance();
        factory.setNamespaceAware(true);
        factory.setXIncludeAware(false);
        factory.setExpandEntityReferences(false);
        // Audiveris's own MusicXML contains the standard MusicXML DOCTYPE. Permit the declaration
        // while disabling every external fetch/entity path; rejecting all DOCTYPEs would reject
        // the artifact we are specifically meant to validate.
        factory.setFeature("http://apache.org/xml/features/nonvalidating/load-external-dtd", false);
        factory.setFeature("http://xml.org/sax/features/external-general-entities", false);
        factory.setFeature("http://xml.org/sax/features/external-parameter-entities", false);
        factory.setAttribute("http://javax.xml.XMLConstants/property/accessExternalDTD", "");
        factory.setAttribute("http://javax.xml.XMLConstants/property/accessExternalSchema", "");
        return factory.newDocumentBuilder().parse(input);
    }

    private static String sha256 (Path path)
        throws Exception
    {
        final MessageDigest digest = MessageDigest.getInstance("SHA-256");
        try (InputStream input = Files.newInputStream(path)) {
            final byte[] buffer = new byte[64 * 1024];
            int count;
            while ((count = input.read(buffer)) >= 0) {
                if (count > 0) {
                    digest.update(buffer, 0, count);
                }
            }
        }
        return HexFormat.of().formatHex(digest.digest());
    }

    private static void markerSuccess (int sheet,
                                       String format,
                                       Path path,
                                       long bytes,
                                       String sha256,
                                       long elapsedNanos)
    {
        System.out.println(
                PREFIX + "{\"schema\":1,\"engine\":\"java\",\"event\":\"export_finished\""
                        + ",\"stage\":\"PAGE\",\"success\":true,\"sheet\":" + sheet
                        + ",\"format\":" + json(format) + ",\"path\":" + json(path.toString())
                        + ",\"bytes\":" + bytes + ",\"sha256\":" + json(sha256)
                        + ",\"elapsed_ms\":" + elapsedMillis(elapsedNanos) + "}");
        System.out.flush();
    }

    private static void markerFailure (int sheet,
                                       String code,
                                       String message,
                                       long elapsedNanos)
    {
        System.out.println(
                PREFIX + "{\"schema\":1,\"engine\":\"java\",\"event\":\"export_finished\""
                        + ",\"stage\":\"PAGE\",\"success\":false,\"sheet\":" + sheet
                        + ",\"error_code\":" + json(code) + ",\"message\":" + json(message)
                        + ",\"elapsed_ms\":" + elapsedMillis(elapsedNanos) + "}");
        System.out.flush();
    }

    private static String elapsedMillis (long elapsedNanos)
    {
        return String.format(Locale.ROOT, "%.3f", elapsedNanos / 1_000_000.0);
    }

    private static String json (String value)
    {
        final String safe = (value != null) ? value : "";
        final StringBuilder quoted = new StringBuilder(safe.length() + 2);
        quoted.append('"');
        for (int index = 0; index < safe.length(); index++) {
            final char character = safe.charAt(index);
            switch (character) {
            case '"':
                quoted.append("\\\"");
                break;
            case '\\':
                quoted.append("\\\\");
                break;
            case '\b':
                quoted.append("\\b");
                break;
            case '\f':
                quoted.append("\\f");
                break;
            case '\n':
                quoted.append("\\n");
                break;
            case '\r':
                quoted.append("\\r");
                break;
            case '\t':
                quoted.append("\\t");
                break;
            default:
                if (character < 0x20) {
                    quoted.append(String.format(Locale.ROOT, "\\u%04x", (int) character));
                } else {
                    quoted.append(character);
                }
                break;
            }
        }
        quoted.append('"');
        return quoted.toString();
    }

    private static String rootMessage (Throwable ex)
    {
        Throwable cause = ex;
        while ((cause.getCause() != null) && (cause.getCause() != cause)) {
            cause = cause.getCause();
        }
        final String message = cause.getMessage();
        return ((message != null) && !message.isBlank())
                ? message
                : cause.getClass().getSimpleName();
    }

    private static ProbeFailure asProbeFailure (Throwable ex)
    {
        if (ex instanceof ProbeFailure failure) {
            return failure;
        }
        return failure("internal_error", rootMessage(ex), ex);
    }

    private static ProbeFailure failure (String code,
                                         String message)
    {
        return new ProbeFailure(code, message, null);
    }

    private static ProbeFailure failure (String code,
                                         String message,
                                         Throwable cause)
    {
        return new ProbeFailure(code, message, cause);
    }

    private static final class ProbeFailure extends Exception
    {
        final String code;

        ProbeFailure (String code,
                      String message,
                      Throwable cause)
        {
            super(message, cause);
            this.code = code;
        }
    }
}
