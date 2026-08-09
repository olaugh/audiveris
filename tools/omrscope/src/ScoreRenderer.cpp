// SPDX-License-Identifier: AGPL-3.0-or-later
#include "ScoreRenderer.h"

#include <QCollator>
#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QPointer>
#include <QProcess>
#include <QStandardPaths>
#include <QTimer>
#include <QUuid>
#include <QXmlStreamReader>

#include <algorithm>
#include <utility>

namespace omrscope {

namespace {

constexpr int kStopGraceMs = 1'000;
constexpr qint64 kMaximumDiagnosticBytes = 64 * 1024;

bool hasSupportedExtension(const QFileInfo &input)
{
    const QString extension = input.suffix().toLower();
    return extension == QLatin1String("mxl") || extension == QLatin1String("musicxml")
        || extension == QLatin1String("xml");
}

bool isContainedBy(const QString &directory, const QString &path)
{
    const QString base = QDir::cleanPath(directory);
    const QString candidate = QDir::cleanPath(path);
    return candidate.startsWith(base + QLatin1Char('/'));
}

QString truncateDiagnostics(const QByteArray &text)
{
    if (text.size() <= kMaximumDiagnosticBytes) {
        return QString::fromUtf8(text).trimmed();
    }
    return QString::fromUtf8(text.first(kMaximumDiagnosticBytes)).trimmed()
        + QStringLiteral("\n[diagnostics truncated]");
}

bool isSvg(const QString &path)
{
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly)) {
        return false;
    }
    QXmlStreamReader xml(&file);
    bool sawDocumentElement = false;
    bool rootIsSvg = false;
    while (!xml.atEnd()) {
        if (xml.readNext() == QXmlStreamReader::StartElement && !sawDocumentElement) {
            sawDocumentElement = true;
            rootIsSvg = xml.name() == QLatin1StringView("svg");
        }
    }
    return sawDocumentElement && rootIsSvg && !xml.hasError();
}

QString explicitRendererError()
{
    const QString override = qEnvironmentVariable("OMRSCOPE_VEROVIO");
    if (!override.isEmpty()) {
        return QStringLiteral("OMRSCOPE_VEROVIO does not name an executable: %1").arg(override);
    }
    return QStringLiteral("Verovio was not found on PATH; install Verovio or set OMRSCOPE_VEROVIO");
}

} // namespace

ScoreRenderer::ScoreRenderer(QObject *parent)
    : QObject(parent)
    , timeoutTimer_(new QTimer(this))
{
    timeoutTimer_->setSingleShot(true);
    connect(timeoutTimer_, &QTimer::timeout, this, &ScoreRenderer::timeout);
}

QString ScoreRenderer::findVerovioExecutable()
{
    const QString override = qEnvironmentVariable("OMRSCOPE_VEROVIO");
    QString executable;
    if (!override.isEmpty()) {
        const QFileInfo candidate(override);
        if (candidate.isAbsolute() || override.contains(QLatin1Char('/'))) {
            executable = candidate.absoluteFilePath();
        } else {
            executable = QStandardPaths::findExecutable(override);
        }
    } else {
        executable = QStandardPaths::findExecutable(QStringLiteral("verovio"));
    }

    const QFileInfo info(executable);
    if (executable.isEmpty() || !info.isFile() || !info.isExecutable()) {
        return {};
    }
    const QString canonical = info.canonicalFilePath();
    return canonical.isEmpty() ? info.absoluteFilePath() : canonical;
}

bool ScoreRenderer::isRunning() const
{
    return active_;
}

bool ScoreRenderer::start(const ScoreRenderRequest &request, QString *error)
{
    if (active_) {
        return reject(QStringLiteral("a score rendering request is already running"), error);
    }

    request_ = request;
    result_ = {};
    result_.renderer = QStringLiteral("Verovio");
    inputCanonicalPath_.clear();
    stagingDirectory_.clear();
    cancelRequested_ = false;
    timeoutRequested_ = false;

    const QFileInfo input(request.inputPath);
    if (!input.exists() || !input.isFile() || !hasSupportedExtension(input)) {
        return reject(QStringLiteral("input must be a regular .mxl, .musicxml, or .xml file: %1")
                          .arg(request.inputPath), error);
    }
    inputCanonicalPath_ = input.canonicalFilePath();
    if (inputCanonicalPath_.isEmpty()) {
        return reject(QStringLiteral("could not resolve input file: %1").arg(request.inputPath), error);
    }
    if (request.timeoutMs <= 0) {
        return reject(QStringLiteral("render timeout must be positive"), error);
    }

    const QFileInfo output(request.outputDirectory);
    if (!output.exists() || !output.isDir()) {
        return reject(QStringLiteral("output directory does not exist: %1")
                          .arg(request.outputDirectory), error);
    }
    const QString outputCanonical = output.canonicalFilePath();
    if (outputCanonical.isEmpty()) {
        return reject(QStringLiteral("could not resolve output directory: %1")
                          .arg(request.outputDirectory), error);
    }

    result_.rendererExecutable = findVerovioExecutable();
    if (result_.rendererExecutable.isEmpty()) {
        return reject(explicitRendererError(), error);
    }

    const QString child = QStringLiteral("omrscope-score-%1")
                              .arg(QUuid::createUuid().toString(QUuid::WithoutBraces));
    QDir outputDirectory(outputCanonical);
    if (!outputDirectory.mkdir(child)) {
        return reject(QStringLiteral("could not create renderer output directory in %1")
                          .arg(outputCanonical), error);
    }
    stagingDirectory_ = QFileInfo(outputDirectory.filePath(child)).canonicalFilePath();
    if (stagingDirectory_.isEmpty() || !isContainedBy(outputCanonical, stagingDirectory_)) {
        return reject(QStringLiteral("renderer output directory escaped caller-owned directory"), error);
    }

    active_ = true;
    timeoutTimer_->start(request.timeoutMs);
    beginVersionProbe();
    return true;
}

void ScoreRenderer::cancel()
{
    if (!active_) {
        return;
    }
    cancelRequested_ = true;
    stopProcess();
}

bool ScoreRenderer::reject(QString error, QString *destination)
{
    if (destination) {
        *destination = std::move(error);
    }
    return false;
}

void ScoreRenderer::beginVersionProbe()
{
    phase_ = Phase::Version;
    launch({QStringLiteral("--version")});
}

void ScoreRenderer::beginRender()
{
    phase_ = Phase::Render;
    const QString outputPath = QDir(stagingDirectory_).filePath(QStringLiteral("page.svg"));
    launch({QStringLiteral("--all-pages"), QStringLiteral("--output-to"), QStringLiteral("svg"),
            QStringLiteral("--outfile"), outputPath, inputCanonicalPath_});
}

void ScoreRenderer::launch(const QStringList &arguments)
{
    auto *process = new QProcess(this);
    process_ = process;
    process->setProcessChannelMode(QProcess::SeparateChannels);
    connect(process, &QProcess::errorOccurred, this, &ScoreRenderer::processError);
    connect(process, qOverload<int, QProcess::ExitStatus>(&QProcess::finished), this,
            [this](int exitCode, QProcess::ExitStatus exitStatus) {
                if (phase_ == Phase::Version) {
                    versionFinished(exitCode, exitStatus);
                } else if (phase_ == Phase::Render) {
                    renderFinished(exitCode, exitStatus);
                }
            });
    process->start(result_.rendererExecutable, arguments);
}

void ScoreRenderer::versionFinished(int exitCode, QProcess::ExitStatus exitStatus)
{
    if (!active_) {
        return;
    }
    if (cancelRequested_ || timeoutRequested_) {
        ScoreRenderResult result = result_;
        result.cancelled = cancelRequested_;
        result.timedOut = timeoutRequested_;
        result.error = timeoutRequested_ ? QStringLiteral("score rendering timed out")
                                         : QStringLiteral("score rendering cancelled");
        complete(std::move(result));
        return;
    }
    if (exitStatus != QProcess::NormalExit || exitCode != 0) {
        ScoreRenderResult result = result_;
        result.error = exitStatus == QProcess::CrashExit
            ? QStringLiteral("Verovio version probe crashed")
            : QStringLiteral("Verovio version probe exited with code %1").arg(exitCode);
        const QString diagnostics = processDiagnostics();
        if (!diagnostics.isEmpty()) {
            result.error += QStringLiteral(": %1").arg(diagnostics);
        }
        complete(std::move(result));
        return;
    }

    result_.rendererVersion = processDiagnostics();
    if (result_.rendererVersion.isEmpty()) {
        ScoreRenderResult result = result_;
        result.error = QStringLiteral("Verovio version probe produced no version text");
        complete(std::move(result));
        return;
    }
    if (process_) {
        process_->deleteLater();
        process_ = nullptr;
    }
    beginRender();
}

void ScoreRenderer::renderFinished(int exitCode, QProcess::ExitStatus exitStatus)
{
    if (!active_) {
        return;
    }
    if (cancelRequested_ || timeoutRequested_) {
        ScoreRenderResult result = result_;
        result.cancelled = cancelRequested_;
        result.timedOut = timeoutRequested_;
        result.error = timeoutRequested_ ? QStringLiteral("score rendering timed out")
                                         : QStringLiteral("score rendering cancelled");
        complete(std::move(result));
        return;
    }
    if (exitStatus != QProcess::NormalExit || exitCode != 0) {
        ScoreRenderResult result = result_;
        result.error = exitStatus == QProcess::CrashExit
            ? QStringLiteral("Verovio renderer crashed")
            : QStringLiteral("Verovio renderer exited with code %1").arg(exitCode);
        const QString diagnostics = processDiagnostics();
        if (!diagnostics.isEmpty()) {
            result.error += QStringLiteral(": %1").arg(diagnostics);
        }
        complete(std::move(result));
        return;
    }

    QString validationError;
    if (!collectPages(&validationError)) {
        ScoreRenderResult result = result_;
        result.error = std::move(validationError);
        complete(std::move(result));
        return;
    }
    complete(result_);
}

void ScoreRenderer::processError(QProcess::ProcessError error)
{
    if (!active_ || error != QProcess::FailedToStart) {
        return;
    }
    ScoreRenderResult result = result_;
    result.error = QStringLiteral("could not start Verovio: %1")
                       .arg(process_ ? process_->errorString() : QStringLiteral("unknown error"));
    complete(std::move(result));
}

void ScoreRenderer::timeout()
{
    if (!active_) {
        return;
    }
    timeoutRequested_ = true;
    stopProcess();
}

void ScoreRenderer::stopProcess()
{
    if (!process_ || process_->state() == QProcess::NotRunning) {
        ScoreRenderResult result = result_;
        result.cancelled = cancelRequested_;
        result.timedOut = timeoutRequested_;
        result.error = timeoutRequested_ ? QStringLiteral("score rendering timed out")
                                         : QStringLiteral("score rendering cancelled");
        complete(std::move(result));
        return;
    }
    process_->terminate();
    QPointer<QProcess> process(process_);
    QTimer::singleShot(kStopGraceMs, this, [process] {
        if (process && process->state() != QProcess::NotRunning) {
            process->kill();
        }
    });
}

bool ScoreRenderer::collectPages(QString *error)
{
    QStringList pages;
    QDirIterator iterator(stagingDirectory_, QDir::Files | QDir::NoDotAndDotDot | QDir::System,
                           QDirIterator::Subdirectories);
    while (iterator.hasNext()) {
        iterator.next();
        const QFileInfo info = iterator.fileInfo();
        if (info.suffix().compare(QLatin1String("svg"), Qt::CaseInsensitive) != 0) {
            continue;
        }
        if (info.isSymLink()) {
            *error = QStringLiteral("renderer emitted a symlink instead of an SVG page: %1")
                         .arg(info.absoluteFilePath());
            return false;
        }
        const QString canonical = info.canonicalFilePath();
        if (canonical.isEmpty() || !isContainedBy(stagingDirectory_, canonical)) {
            *error = QStringLiteral("renderer emitted an SVG outside its output directory: %1")
                         .arg(info.absoluteFilePath());
            return false;
        }
        if (info.size() <= 0 || !isSvg(canonical)) {
            *error = QStringLiteral("renderer emitted an empty or invalid SVG page: %1").arg(canonical);
            return false;
        }
        pages.append(canonical);
    }

    if (pages.isEmpty()) {
        *error = QStringLiteral("Verovio completed without producing SVG pages");
        return false;
    }
    QCollator collator;
    collator.setNumericMode(true);
    std::sort(pages.begin(), pages.end(), [&collator](const QString &left, const QString &right) {
        return collator.compare(left, right) < 0;
    });
    result_.pagePaths = std::move(pages);
    return true;
}

void ScoreRenderer::complete(ScoreRenderResult result)
{
    if (!active_) {
        return;
    }
    active_ = false;
    phase_ = Phase::Idle;
    timeoutTimer_->stop();
    if (process_) {
        process_->disconnect(this);
        process_->deleteLater();
        process_ = nullptr;
    }
    emit finished(result);
}

QString ScoreRenderer::processDiagnostics() const
{
    if (!process_) {
        return {};
    }
    const QByteArray diagnostics = process_->readAllStandardOutput()
        + process_->readAllStandardError();
    return truncateDiagnostics(diagnostics);
}

} // namespace omrscope
