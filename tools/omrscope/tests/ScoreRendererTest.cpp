// SPDX-License-Identifier: AGPL-3.0-or-later
#include "ScoreRenderer.h"

#include <QCoreApplication>
#include <QDir>
#include <QEventLoop>
#include <QFile>
#include <QFileInfo>
#include <QTemporaryDir>
#include <QTextStream>
#include <QThread>
#include <QTimer>

namespace {

bool check(bool condition, const char *message)
{
    if (!condition) {
        QTextStream(stderr) << "FAIL: " << message << '\n';
    }
    return condition;
}

bool installExecutable(const QString &target)
{
    QFile::remove(target);
    if (!QFile::copy(QCoreApplication::applicationFilePath(), target)) {
        return false;
    }
    return QFile::setPermissions(target, QFileDevice::ReadOwner | QFileDevice::WriteOwner
                                     | QFileDevice::ExeOwner);
}

bool writeFile(const QString &path, const QByteArray &contents)
{
    QFile file(path);
    return file.open(QIODevice::WriteOnly) && file.write(contents) == contents.size();
}

QByteArray minimalMusicXml()
{
    return QByteArrayLiteral(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
        "<!DOCTYPE score-partwise PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" "
        "\"http://www.musicxml.org/dtds/partwise.dtd\">\n"
        "<score-partwise version=\"4.0\"><part-list><score-part id=\"P1\">"
        "<part-name>Music</part-name></score-part></part-list><part id=\"P1\">"
        "<measure number=\"1\"><attributes><divisions>1</divisions><key><fifths>0</fifths>"
        "</key><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign>"
        "<line>2</line></clef></attributes><note><rest/><duration>4</duration><type>whole</type>"
        "</note></measure></part></score-partwise>");
}

QString fakeOutputPath(const QStringList &arguments)
{
    for (qsizetype index = 0; index + 1 < arguments.size(); ++index) {
        if (arguments.at(index) == QLatin1String("--outfile")) {
            return arguments.at(index + 1);
        }
    }
    return {};
}

int fakeRenderer(const QStringList &arguments)
{
    const QString mode = qEnvironmentVariable("OMRSCOPE_SCORE_RENDERER_FAKE_MODE");
    if (arguments.contains(QStringLiteral("--version"))) {
        if (mode == QLatin1String("version-failure")) {
            QTextStream(stderr) << "synthetic version failure\n";
            return 17;
        }
        QTextStream(stdout) << "Verovio 6.2.1-fake\n";
        return 0;
    }
    if (mode == QLatin1String("sleep")) {
        QThread::msleep(2'000);
        return 0;
    }
    if (mode == QLatin1String("nonzero")) {
        QTextStream(stderr) << "synthetic render failure\n";
        return 9;
    }
    if (mode == QLatin1String("no-output")) {
        return 0;
    }

    const QString output = fakeOutputPath(arguments);
    if (output.isEmpty()) {
        QTextStream(stderr) << "missing --outfile\n";
        return 8;
    }
    if (mode == QLatin1String("nested-svg")) {
        return writeFile(output, QByteArrayLiteral(
            "<html xmlns=\"http://www.w3.org/1999/xhtml\">"
            "<svg xmlns=\"http://www.w3.org/2000/svg\"/></html>")) ? 0 : 12;
    }
    if (mode == QLatin1String("uppercase-svg")) {
        return writeFile(output, QByteArrayLiteral(
            "<SVG xmlns=\"http://www.w3.org/2000/svg\"/>")) ? 0 : 14;
    }
    if (mode == QLatin1String("truncated-svg")) {
        return writeFile(output, QByteArrayLiteral(
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><g>")) ? 0 : 13;
    }
    if (mode == QLatin1String("unsafe-output")) {
        const QString outside = QDir::temp().filePath(QStringLiteral("omrscope-score-renderer-outside.svg"));
        if (!writeFile(outside, QByteArrayLiteral("<svg xmlns=\"http://www.w3.org/2000/svg\"/>"))
            || !QFile::link(outside, output)) {
            return 11;
        }
        return 0;
    }
    if (!writeFile(output, QByteArrayLiteral("<svg xmlns=\"http://www.w3.org/2000/svg\"><g/></svg>"))) {
        return 10;
    }
    return 0;
}

struct Observation {
    int completions = 0;
    omrscope::ScoreRenderResult result;
};

Observation render(omrscope::ScoreRenderer &renderer, const omrscope::ScoreRenderRequest &request,
                   int cancelAfterMs = -1)
{
    Observation observation;
    QEventLoop loop;
    const auto connection = QObject::connect(
        &renderer, &omrscope::ScoreRenderer::finished, &loop,
        [&](const omrscope::ScoreRenderResult &result) {
            ++observation.completions;
            observation.result = result;
            loop.quit();
        });
    if (!renderer.start(request)) {
        return observation;
    }
    if (cancelAfterMs >= 0) {
        QTimer::singleShot(cancelAfterMs, &renderer, &omrscope::ScoreRenderer::cancel);
    }
    QTimer::singleShot(5'000, &loop, &QEventLoop::quit);
    loop.exec();
    QObject::disconnect(connection);
    return observation;
}

} // namespace

int main(int argc, char **argv)
{
    QCoreApplication application(argc, argv);
    if (!qEnvironmentVariableIsEmpty("OMRSCOPE_SCORE_RENDERER_FAKE_MODE")) {
        return fakeRenderer(QCoreApplication::arguments());
    }

    QTemporaryDir temporary;
    if (!temporary.isValid()) {
        return 1;
    }
    const QString fake = QDir(temporary.path()).filePath(QStringLiteral("verovio-fake"));
    const QString musicXml = QDir(temporary.path()).filePath(QStringLiteral("score.musicxml"));
    const QString xml = QDir(temporary.path()).filePath(QStringLiteral("score.xml"));
    const QString output = QDir(temporary.path()).filePath(QStringLiteral("output"));
    bool ok = true;
    ok &= check(installExecutable(fake), "self executable fake renderer is installed");
    ok &= check(writeFile(musicXml, minimalMusicXml()),
                "minimal MusicXML input is written");
    ok &= check(writeFile(xml, minimalMusicXml()),
                "conventional .xml MusicXML input is written");
    ok &= check(QDir().mkpath(output), "caller-owned output directory exists");
    if (!ok) {
        return 1;
    }

    omrscope::ScoreRenderer renderer;
    omrscope::ScoreRenderRequest request{musicXml, output, 1'000};
    qputenv("OMRSCOPE_VEROVIO", fake.toUtf8());

    qputenv("OMRSCOPE_SCORE_RENDERER_FAKE_MODE", QByteArrayLiteral("success"));
    const Observation success = render(renderer, request);
    ok &= check(success.completions == 1 && success.result.succeeded()
                    && success.result.renderer == QLatin1String("Verovio")
                    && success.result.rendererVersion.contains(QStringLiteral("6.2.1-fake"))
                    && success.result.pagePaths.size() == 1
                    && QFileInfo(success.result.pagePaths.front()).size() > 0,
                "fake Verovio succeeds with a validated SVG page and version");

    const Observation conventionalXml = render(renderer, {xml, output, 1'000});
    ok &= check(conventionalXml.completions == 1 && conventionalXml.result.succeeded(),
                "conventional .xml MusicXML output is accepted");

    qputenv("OMRSCOPE_SCORE_RENDERER_FAKE_MODE", QByteArrayLiteral("nonzero"));
    const Observation nonzero = render(renderer, request);
    ok &= check(nonzero.completions == 1 && !nonzero.result.succeeded()
                    && nonzero.result.error.contains(QStringLiteral("code 9"))
                    && nonzero.result.error.contains(QStringLiteral("synthetic render failure")),
                "nonzero renderer exit preserves diagnostics");

    qputenv("OMRSCOPE_SCORE_RENDERER_FAKE_MODE", QByteArrayLiteral("no-output"));
    const Observation noOutput = render(renderer, request);
    ok &= check(noOutput.completions == 1 && !noOutput.result.succeeded()
                    && noOutput.result.error.contains(QStringLiteral("without producing SVG")),
                "zero-exit renderer without an SVG fails validation");

    qputenv("OMRSCOPE_SCORE_RENDERER_FAKE_MODE", QByteArrayLiteral("nested-svg"));
    const Observation nestedSvg = render(renderer, request);
    ok &= check(nestedSvg.completions == 1 && !nestedSvg.result.succeeded()
                    && nestedSvg.result.error.contains(QStringLiteral("invalid SVG")),
                "an SVG nested below a non-SVG document element is rejected");

    qputenv("OMRSCOPE_SCORE_RENDERER_FAKE_MODE", QByteArrayLiteral("uppercase-svg"));
    const Observation uppercaseSvg = render(renderer, request);
    ok &= check(uppercaseSvg.completions == 1 && !uppercaseSvg.result.succeeded()
                    && uppercaseSvg.result.error.contains(QStringLiteral("invalid SVG")),
                "the case-sensitive SVG document element must be lower-case svg");

    qputenv("OMRSCOPE_SCORE_RENDERER_FAKE_MODE", QByteArrayLiteral("truncated-svg"));
    const Observation truncatedSvg = render(renderer, request);
    ok &= check(truncatedSvg.completions == 1 && !truncatedSvg.result.succeeded()
                    && truncatedSvg.result.error.contains(QStringLiteral("invalid SVG")),
                "a truncated SVG document is rejected after parsing to EOF");

    qputenv("OMRSCOPE_SCORE_RENDERER_FAKE_MODE", QByteArrayLiteral("unsafe-output"));
    const Observation unsafeOutput = render(renderer, request);
    ok &= check(unsafeOutput.completions == 1 && !unsafeOutput.result.succeeded()
                    && unsafeOutput.result.error.contains(QStringLiteral("symlink")),
                "SVG symlink escaping the staging directory is rejected");

    qputenv("OMRSCOPE_SCORE_RENDERER_FAKE_MODE", QByteArrayLiteral("sleep"));
    Observation cancelled;
    QEventLoop cancelLoop;
    const auto cancellationConnection = QObject::connect(
        &renderer, &omrscope::ScoreRenderer::finished, &cancelLoop,
        [&](const omrscope::ScoreRenderResult &result) {
            ++cancelled.completions;
            cancelled.result = result;
            cancelLoop.quit();
        });
    const bool firstStarted = renderer.start(request);
    QString secondStartError;
    const bool secondStarted = renderer.start(request, &secondStartError);
    QTimer::singleShot(20, &renderer, &omrscope::ScoreRenderer::cancel);
    QTimer::singleShot(5'000, &cancelLoop, &QEventLoop::quit);
    cancelLoop.exec();
    QObject::disconnect(cancellationConnection);
    ok &= check(firstStarted && !secondStarted
                    && secondStartError.contains(QStringLiteral("already running"))
                    && cancelled.completions == 1 && cancelled.result.cancelled
                    && !cancelled.result.timedOut,
                "second start is synchronously rejected while the original has one cancellation result");

    omrscope::ScoreRenderRequest fastTimeout{musicXml, output, 20};
    const Observation timedOut = render(renderer, fastTimeout);
    ok &= check(timedOut.completions == 1 && timedOut.result.timedOut
                    && !timedOut.result.cancelled,
                "timeout terminates a running renderer and reports timeout once");

    qputenv("OMRSCOPE_VEROVIO", QByteArrayLiteral("/definitely/not/a/verovio"));
    qputenv("OMRSCOPE_SCORE_RENDERER_FAKE_MODE", QByteArrayLiteral("success"));
    QString missingError;
    const bool missingStarted = renderer.start(request, &missingError);
    ok &= check(!missingStarted && missingError.contains(QStringLiteral("OMRSCOPE_VEROVIO")),
                "missing explicit renderer fails closed instead of falling back to PATH");

    qunsetenv("OMRSCOPE_VEROVIO");
    qunsetenv("OMRSCOPE_SCORE_RENDERER_FAKE_MODE");

    QTemporaryDir realOutput;
    const QString installed = omrscope::ScoreRenderer::findVerovioExecutable();
    if (realOutput.isValid() && !installed.isEmpty()) {
        omrscope::ScoreRenderer realRenderer;
        const Observation real = render(
            realRenderer, {musicXml, realOutput.path(), 10'000});
        ok &= check(real.completions == 1 && real.result.succeeded()
                        && real.result.rendererVersion.contains(QStringLiteral("Verovio"))
                        && !real.result.pagePaths.isEmpty(),
                    "installed Verovio renders a minimal valid MusicXML document");
    }

    QFile::remove(QDir::temp().filePath(QStringLiteral("omrscope-score-renderer-outside.svg")));
    return ok ? 0 : 1;
}
