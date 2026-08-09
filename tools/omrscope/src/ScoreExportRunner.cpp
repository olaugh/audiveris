// SPDX-License-Identifier: AGPL-3.0-or-later
#include "ScoreExportRunner.h"

#include <QCryptographicHash>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>
#include <QProcessEnvironment>
#include <QRegularExpression>

#include <cmath>
#include <cstring>
#include <limits>

#ifdef Q_OS_UNIX
#include <cerrno>
#include <csignal>
#include <sys/types.h>
#endif

namespace omrscope {

namespace {

constexpr int kExportTimeoutMs = 30 * 60 * 1000;
constexpr qsizetype kMaxDiagnosticChars = 64 * 1024;
const QString kPrefix = QStringLiteral("@omrscope-export ");

bool exactInteger(const QJsonValue &value, qint64 minimum, qint64 *result)
{
    if (!value.isDouble()) {
        return false;
    }
    const double number = value.toDouble();
    // JSON numbers are doubles. Stay inside the exact-integer range before casting.
    constexpr double largestExactInteger = 9007199254740991.0; // 2^53 - 1
    if (!std::isfinite(number) || number < static_cast<double>(minimum)
        || number > largestExactInteger
        || std::floor(number) != number) {
        return false;
    }
    *result = static_cast<qint64>(number);
    return true;
}

std::optional<ScoreExportResult> markerError(const QString &message, QString *error)
{
    if (error) {
        *error = message;
    }
    return std::nullopt;
}

QString sha256(const QString &path)
{
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly)) {
        return {};
    }
    QCryptographicHash hash(QCryptographicHash::Sha256);
    if (!hash.addData(&file)) {
        return {};
    }
    return QString::fromLatin1(hash.result().toHex());
}

QString formatForPath(const QString &path)
{
    // Audiveris chooses its exporter from the literal destination suffix. In particular, `.MXL`
    // is not an alias for `.mxl`; accepting it here would let the probe and this validator
    // disagree about the requested artifact. Keep the preflight contract deliberately exact.
    const QString suffix = QFileInfo(path).suffix();
    if (suffix == QLatin1String("mxl")) {
        return QStringLiteral("mxl");
    }
    if (suffix == QLatin1String("xml")) {
        return QStringLiteral("musicxml");
    }
    return {};
}

} // namespace

std::optional<ScoreExportResult> parseScoreExportMarker(const QString &line, QString *error)
{
    if (error) {
        error->clear();
    }
    if (!line.startsWith(kPrefix)) {
        return markerError(QStringLiteral("missing @omrscope-export prefix"), error);
    }

    QJsonParseError parseError;
    const QJsonDocument document =
        QJsonDocument::fromJson(line.mid(kPrefix.size()).toUtf8(), &parseError);
    if (parseError.error != QJsonParseError::NoError || !document.isObject()) {
        return markerError(QStringLiteral("invalid export JSON: %1").arg(parseError.errorString()),
                           error);
    }
    const QJsonObject object = document.object();
    qint64 schema = 0;
    if (!exactInteger(object.value(QStringLiteral("schema")), 1, &schema) || schema != 1) {
        return markerError(QStringLiteral("unsupported export schema"), error);
    }
    if (object.value(QStringLiteral("engine")).toString() != QLatin1String("java")) {
        return markerError(QStringLiteral("export marker engine is not java"), error);
    }
    if (object.value(QStringLiteral("event")).toString()
        != QLatin1String("export_finished")) {
        return markerError(QStringLiteral("unknown export event"), error);
    }
    if (object.value(QStringLiteral("stage")).toString() != QLatin1String("PAGE")) {
        return markerError(QStringLiteral("export marker stage is not PAGE"), error);
    }
    if (!object.value(QStringLiteral("success")).isBool()) {
        return markerError(QStringLiteral("export marker has no boolean success"), error);
    }

    qint64 sheet = 0;
    if (!exactInteger(object.value(QStringLiteral("sheet")), 0, &sheet)
        || sheet > std::numeric_limits<int>::max()) {
        return markerError(QStringLiteral("export marker has an invalid sheet"), error);
    }
    const QJsonValue elapsedValue = object.value(QStringLiteral("elapsed_ms"));
    if (!elapsedValue.isDouble() || !std::isfinite(elapsedValue.toDouble())
        || elapsedValue.toDouble() < 0.0) {
        return markerError(QStringLiteral("export marker has invalid elapsed_ms"), error);
    }

    ScoreExportResult result;
    result.success = object.value(QStringLiteral("success")).toBool();
    result.sheet = static_cast<int>(sheet);
    result.elapsedMillis = elapsedValue.toDouble();

    if (result.success) {
        if (result.sheet <= 0) {
            return markerError(QStringLiteral("successful export has no sheet"), error);
        }
        result.format = object.value(QStringLiteral("format")).toString();
        if (result.format != QLatin1String("mxl")
            && result.format != QLatin1String("musicxml")) {
            return markerError(QStringLiteral("successful export has invalid format"), error);
        }
        result.path = object.value(QStringLiteral("path")).toString();
        if (result.path.isEmpty() || !QDir::isAbsolutePath(result.path)) {
            return markerError(QStringLiteral("successful export path is not absolute"), error);
        }
        qint64 bytes = 0;
        if (!exactInteger(object.value(QStringLiteral("bytes")), 1, &bytes)) {
            return markerError(QStringLiteral("successful export has invalid bytes"), error);
        }
        result.bytes = bytes;
        result.sha256 = object.value(QStringLiteral("sha256")).toString();
        static const QRegularExpression digest(QStringLiteral("^[0-9a-f]{64}$"));
        if (!digest.match(result.sha256).hasMatch()) {
            return markerError(QStringLiteral("successful export has invalid sha256"), error);
        }
        if (object.contains(QStringLiteral("error_code"))
            || object.contains(QStringLiteral("message"))) {
            return markerError(QStringLiteral("successful export carries failure fields"), error);
        }
    } else {
        result.errorCode = object.value(QStringLiteral("error_code")).toString();
        result.message = object.value(QStringLiteral("message")).toString();
        static const QRegularExpression code(QStringLiteral("^[a-z][a-z0-9_]*$"));
        if (!code.match(result.errorCode).hasMatch() || result.message.isEmpty()) {
            return markerError(QStringLiteral("failed export has invalid error fields"), error);
        }
        if (object.contains(QStringLiteral("path")) || object.contains(QStringLiteral("bytes"))
            || object.contains(QStringLiteral("sha256"))) {
            return markerError(QStringLiteral("failed export carries artifact fields"), error);
        }
    }

    return result;
}

ScoreExportRunner::ScoreExportRunner(QDir repository, QObject *parent)
    : QObject(parent)
    , repository_(std::move(repository))
    , javaHome_(findJavaHome(repository_))
{
    qRegisterMetaType<omrscope::ScoreExportResult>();
    timeout_.setSingleShot(true);
    connect(&timeout_, &QTimer::timeout, this, [this] {
        forceFailure(QStringLiteral("timeout"),
                     QStringLiteral("Java PAGE export exceeded 30 minutes"));
    });
}

ScoreExportRunner::~ScoreExportRunner()
{
    if (process_ && process_->state() != QProcess::NotRunning) {
        stopProcessTree();
        process_->waitForFinished(5000);
    }
}

QString ScoreExportRunner::findJavaHome(const QDir &repository)
{
    const QString fromEnvironment = qEnvironmentVariable("JAVA_HOME");
    if (!fromEnvironment.isEmpty() && QFileInfo::exists(fromEnvironment + "/bin/java")) {
        return fromEnvironment;
    }
    const QStringList candidates = {
        repository.filePath(QStringLiteral("tools/jdk25/Contents/Home")),
        QStringLiteral("/Users/john/sources/jul10-charter/omr/tools/jdk25/Contents/Home"),
    };
    for (const QString &candidate : candidates) {
        if (QFileInfo::exists(candidate + "/bin/java")) {
            return candidate;
        }
    }
    return {};
}

bool ScoreExportRunner::isRunning() const
{
    return process_ && process_->state() != QProcess::NotRunning;
}

bool ScoreExportRunner::startJava(const QString &input, int sheet, const QString &output,
                                  ScoreExportResult *error)
{
    if (isRunning()) {
        return rejectStart(sheet, QStringLiteral("busy"),
                           QStringLiteral("a score export is already running"), error);
    }
    clearProcess();
    expectedSheet_ = sheet;

    if (javaHome_.isEmpty()) {
        return rejectStart(sheet, QStringLiteral("jdk_missing"),
                           QStringLiteral("no JDK found; set JAVA_HOME to a JDK 25"), error);
    }
    if (sheet <= 0) {
        return rejectStart(sheet, QStringLiteral("invalid_arguments"),
                           QStringLiteral("sheet must be a positive integer"), error);
    }

    const QString absoluteInput = QFileInfo(input).isAbsolute()
        ? QFileInfo(input).absoluteFilePath()
        : QFileInfo(repository_.filePath(input)).absoluteFilePath();
    const QFileInfo inputInfo(absoluteInput);
    if (!inputInfo.isFile()) {
        return rejectStart(sheet, QStringLiteral("input_missing"),
                           QStringLiteral("input is not a regular file: %1").arg(absoluteInput),
                           error);
    }
    if (!QDir::isAbsolutePath(output)) {
        return rejectStart(sheet, QStringLiteral("invalid_arguments"),
                           QStringLiteral("output path must be absolute"), error);
    }
    if (formatForPath(output).isEmpty()) {
        return rejectStart(sheet, QStringLiteral("invalid_arguments"),
                           QStringLiteral("output extension must be .mxl or .xml"), error);
    }
    const QFileInfo outputInfo(output);
    if (outputInfo.exists() || outputInfo.isSymLink()) {
        return rejectStart(sheet, QStringLiteral("output_exists"),
                           QStringLiteral("output already exists: %1").arg(output), error);
    }
    const QDir parent(outputInfo.absolutePath());
    if (!parent.exists() || parent.canonicalPath().isEmpty()) {
        return rejectStart(
            sheet, QStringLiteral("invalid_arguments"),
            QStringLiteral("output directory does not exist: %1").arg(parent.path()), error);
    }
    expectedOutput_ = QDir(parent.canonicalPath()).filePath(outputInfo.fileName());

    pendingStdout_.clear();
    diagnostics_.clear();
    marker_.reset();
    forcedErrorCode_.clear();
    forcedMessage_.clear();
    terminalEmitted_ = false;

    process_ = new QProcess(this);
    process_->setWorkingDirectory(repository_.absolutePath());
    process_->setProcessChannelMode(QProcess::SeparateChannels);
#ifdef Q_OS_UNIX
    // Qt arranges setsid() in the child before exec. The Gradle launcher is therefore both the
    // process-group and session leader, and JavaExec descendants remain in its killable group.
    // A failure to create the session makes QProcess report FailedToStart rather than silently
    // falling back to a launcher-only cancellation.
    process_->setUnixProcessParameters(QProcess::UnixProcessFlag::CreateNewSession);
#endif
    QProcessEnvironment environment = QProcessEnvironment::systemEnvironment();
    environment.insert(QStringLiteral("JAVA_HOME"), javaHome_);
    environment.remove(QStringLiteral("JAVA_TOOL_OPTIONS"));
    process_->setProcessEnvironment(environment);

    connect(process_, &QProcess::readyReadStandardOutput, this,
            &ScoreExportRunner::consumeStdout);
    connect(process_, &QProcess::readyReadStandardError, this, [this] {
        if (process_) {
            appendDiagnostic(QString::fromUtf8(process_->readAllStandardError()));
        }
    });
    connect(process_, &QProcess::errorOccurred, this, [this](QProcess::ProcessError error) {
        if (error == QProcess::FailedToStart && process_) {
            const QString message =
                QStringLiteral("could not start Gradle: %1").arg(process_->errorString());
            forcedErrorCode_ = QStringLiteral("process_start_failed");
            forcedMessage_ = message;
            emitFailure(forcedErrorCode_, forcedMessage_);
            clearProcess();
        }
    });
    connect(process_, qOverload<int, QProcess::ExitStatus>(&QProcess::finished), this,
            &ScoreExportRunner::processFinished);

    const QStringList arguments{
        QStringLiteral("--no-daemon"),
        QStringLiteral("-q"),
        QStringLiteral("-I"),
        QStringLiteral("rust/oracle/java/staff-impacts.init.gradle"),
        QStringLiteral(":app:musicXmlExportProbe"),
        QStringLiteral("-PmusicXmlExportInput=%1").arg(inputInfo.canonicalFilePath()),
        QStringLiteral("-PmusicXmlExportSheet=%1").arg(sheet),
        QStringLiteral("-PmusicXmlExportOutput=%1").arg(expectedOutput_),
    };
    process_->start(repository_.filePath(QStringLiteral("gradlew")), arguments);
    timeout_.start(kExportTimeoutMs);
    return true;
}

void ScoreExportRunner::cancel()
{
    if (!isRunning()) {
        return;
    }
    forceFailure(QStringLiteral("cancelled"), QStringLiteral("score export cancelled"));
}

void ScoreExportRunner::consumeStdout()
{
    if (!process_) {
        return;
    }
    pendingStdout_ += process_->readAllStandardOutput();
    while (true) {
        const qsizetype newline = pendingStdout_.indexOf('\n');
        if (newline < 0) {
            break;
        }
        QByteArray bytes = pendingStdout_.left(newline);
        pendingStdout_.remove(0, newline + 1);
        if (bytes.endsWith('\r')) {
            bytes.chop(1);
        }
        consumeLine(QString::fromUtf8(bytes));
    }
}

void ScoreExportRunner::consumeLine(const QString &line)
{
    if (!line.startsWith(kPrefix)) {
        appendDiagnostic(line + QLatin1Char('\n'));
        return;
    }
    if (marker_) {
        forceFailure(QStringLiteral("protocol_error"),
                     QStringLiteral("received more than one export_finished marker"));
        return;
    }
    QString error;
    const auto parsed = parseScoreExportMarker(line, &error);
    if (!parsed) {
        forceFailure(QStringLiteral("protocol_error"), error);
        return;
    }
    if (parsed->sheet != expectedSheet_ && parsed->sheet != 0) {
        forceFailure(QStringLiteral("protocol_error"),
                     QStringLiteral("export marker sheet %1 does not match requested sheet %2")
                         .arg(parsed->sheet)
                         .arg(expectedSheet_));
        return;
    }
    marker_ = *parsed;
}

void ScoreExportRunner::appendDiagnostic(const QString &text)
{
    if (text.isEmpty()) {
        return;
    }
    diagnostics_ += text;
    if (diagnostics_.size() > kMaxDiagnosticChars) {
        diagnostics_.remove(0, diagnostics_.size() - kMaxDiagnosticChars);
    }
}

void ScoreExportRunner::forceFailure(const QString &code, const QString &message)
{
    if (forcedErrorCode_.isEmpty()) {
        forcedErrorCode_ = code;
        forcedMessage_ = message;
    }
    if (process_ && process_->state() != QProcess::NotRunning) {
        stopProcessTree();
    }
}

void ScoreExportRunner::stopProcessTree()
{
    if (!process_ || process_->state() == QProcess::NotRunning) {
        return;
    }

#ifdef Q_OS_UNIX
    // `CreateNewSession` above makes the launcher's PID its process-group ID. Negative PID is
    // the POSIX process-group form of kill(2), so the Gradle wrapper and every JavaExec child are
    // stopped together before QProcess can report the launcher terminal. ESRCH merely means the
    // group was already reaped; retain QProcess::kill below for the narrow startup race.
    const qint64 processId = process_->processId();
    if (processId > 0) {
        errno = 0;
        if (::kill(-static_cast<pid_t>(processId), SIGKILL) == 0) {
            return;
        }
        // ESRCH is also possible before the child has completed setsid(). In that narrow startup
        // race, fall through to the direct QProcess kill instead of assuming the launcher exited.
        if (errno != ESRCH) {
            appendDiagnostic(QStringLiteral("could not terminate Java export process group: %1\n")
                                 .arg(QString::fromLocal8Bit(std::strerror(errno))));
        }
    }
#endif

#ifdef Q_OS_WIN
    // Windows has no QProcess equivalent of a Unix process group. `taskkill /T` is the native
    // tree operation: it waits for the wrapper and its descendants rather than leaving JavaExec
    // running after Gradle itself has exited. Keep the direct-QProcess kill below only as a
    // failure fallback (for example when taskkill is unavailable on a constrained installation).
    const qint64 processId = process_->processId();
    if (processId > 0) {
        QProcess taskkill;
        taskkill.setProcessChannelMode(QProcess::MergedChannels);
        taskkill.start(QStringLiteral("taskkill"),
                       {QStringLiteral("/PID"), QString::number(processId),
                        QStringLiteral("/T"), QStringLiteral("/F")});
        if (taskkill.waitForFinished(5000) && taskkill.exitStatus() == QProcess::NormalExit
            && taskkill.exitCode() == 0) {
            return;
        }
        appendDiagnostic(QStringLiteral("could not terminate Java export process tree: %1\n")
                             .arg(QString::fromUtf8(taskkill.readAll())));
    }
#endif

    // Last-resort fallback for platforms with no native tree primitive. QProcess owns this
    // launcher and will not emit its terminal signal until it has exited.
    process_->kill();
}

void ScoreExportRunner::processFinished(int exitCode, QProcess::ExitStatus status)
{
    timeout_.stop();
    if (process_) {
        consumeStdout();
        appendDiagnostic(QString::fromUtf8(process_->readAllStandardError()));
    }
    if (!pendingStdout_.isEmpty()) {
        QByteArray bytes = pendingStdout_;
        pendingStdout_.clear();
        if (bytes.endsWith('\r')) {
            bytes.chop(1);
        }
        consumeLine(QString::fromUtf8(bytes));
    }

    if (!forcedErrorCode_.isEmpty()) {
        emitFailure(forcedErrorCode_, forcedMessage_);
        clearProcess();
        return;
    }
    if (!marker_) {
        emitFailure(QStringLiteral("protocol_error"),
                    QStringLiteral("Java export emitted no export_finished marker"));
        clearProcess();
        return;
    }

    ScoreExportResult result = *marker_;
    result.diagnostics = diagnostics_.trimmed();
    if (result.success) {
        if (status != QProcess::NormalExit || exitCode != 0) {
            emitFailure(QStringLiteral("process_failed"),
                        status == QProcess::CrashExit
                            ? QStringLiteral("Gradle export process crashed")
                            : QStringLiteral("Gradle export exited with code %1").arg(exitCode));
            clearProcess();
            return;
        }
        QString code;
        QString message;
        const auto validated = validateArtifact(result, &code, &message);
        if (!validated) {
            emitFailure(code, message);
            clearProcess();
            return;
        }
        emitOnce(*validated);
        clearProcess();
        return;
    }

    if (status != QProcess::NormalExit) {
        emitFailure(QStringLiteral("process_failed"),
                    QStringLiteral("Gradle export process crashed after a failure marker"));
    } else if (exitCode == 0) {
        emitFailure(QStringLiteral("protocol_error"),
                    QStringLiteral("failed export marker was followed by a zero exit"));
    } else {
        emitOnce(result);
    }
    clearProcess();
}

std::optional<ScoreExportResult> ScoreExportRunner::validateArtifact(
    ScoreExportResult result, QString *errorCode, QString *errorMessage) const
{
    const QFileInfo file(expectedOutput_);
    if (!file.isFile() || file.isSymLink()) {
        *errorCode = QStringLiteral("artifact_missing");
        *errorMessage = QStringLiteral("requested export is not a regular local file");
        return std::nullopt;
    }
    const QString canonical = file.canonicalFilePath();
    if (canonical.isEmpty() || result.path != canonical) {
        *errorCode = QStringLiteral("artifact_mismatch");
        *errorMessage = QStringLiteral("probe reported an unexpected export path");
        return std::nullopt;
    }
    const QString format = formatForPath(canonical);
    if (result.format != format) {
        *errorCode = QStringLiteral("artifact_mismatch");
        *errorMessage = QStringLiteral("probe format does not match the requested extension");
        return std::nullopt;
    }
    if (file.size() <= 0 || file.size() != result.bytes) {
        *errorCode = QStringLiteral("artifact_mismatch");
        *errorMessage = QStringLiteral("probe byte count does not match the local export");
        return std::nullopt;
    }
    const QString digest = sha256(canonical);
    if (digest.isEmpty() || digest != result.sha256) {
        *errorCode = QStringLiteral("artifact_mismatch");
        *errorMessage = QStringLiteral("probe SHA-256 does not match the local export");
        return std::nullopt;
    }
    result.path = canonical;
    result.bytes = file.size();
    result.sha256 = digest;
    result.diagnostics = diagnostics_.trimmed();
    return result;
}

void ScoreExportRunner::emitFailure(const QString &code, const QString &message)
{
    ScoreExportResult result;
    result.sheet = expectedSheet_;
    result.errorCode = code;
    result.message = message;
    result.diagnostics = diagnostics_.trimmed();
    emitOnce(result);
}

void ScoreExportRunner::emitOnce(ScoreExportResult result)
{
    if (terminalEmitted_) {
        return;
    }
    terminalEmitted_ = true;
    emit exportFinished(std::move(result));
}

bool ScoreExportRunner::rejectStart(int sheet, const QString &code, const QString &message,
                                    ScoreExportResult *error) const
{
    if (error) {
        *error = ScoreExportResult{};
        error->sheet = sheet;
        error->errorCode = code;
        error->message = message;
    }
    return false;
}

void ScoreExportRunner::clearProcess()
{
    timeout_.stop();
    if (process_) {
        process_->deleteLater();
        process_ = nullptr;
    }
    pendingStdout_.clear();
    marker_.reset();
    forcedErrorCode_.clear();
    forcedMessage_.clear();
}

} // namespace omrscope
