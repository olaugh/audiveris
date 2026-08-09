// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include <QByteArray>
#include <QDir>
#include <QObject>
#include <QProcess>
#include <QString>
#include <QTimer>

#include <optional>

namespace omrscope {

/// One terminal PAGE-to-MusicXML attempt.
///
/// Export is Java-only today. Keeping it out of EngineResult avoids implying that the Rust port
/// can emit a score, and keeping it out of StreamEvent prevents an export record from being
/// accepted as recognition-stage framing.
struct ScoreExportResult
{
    bool success = false;
    QString engine = QStringLiteral("java");
    QString stage = QStringLiteral("PAGE");
    int sheet = 0;
    QString format;       ///< "mxl" or "musicxml".
    QString path;         ///< Canonical path, populated only after local revalidation.
    qint64 bytes = 0;
    QString sha256;       ///< Lower-case hexadecimal SHA-256.
    double elapsedMillis = 0.0;
    QString errorCode;
    QString message;
    QString diagnostics; ///< Bounded non-protocol stdout/stderr from Gradle/Audiveris.
};

/// Parse one complete `@omrscope-export ` line. The protocol has exactly one terminal event.
std::optional<ScoreExportResult> parseScoreExportMarker(const QString &line,
                                                        QString *error = nullptr);

/// Asynchronously run Java PAGE recognition and export one sheet to an explicit MusicXML path.
///
/// The caller owns the output directory (normally a per-attempt QTemporaryDir) and must provide
/// an absolute, absent output path ending in `.mxl` or `.xml`. The runner accepts success only
/// when the probe emits one valid marker, exits zero, and the requested local file independently
/// matches the marker's canonical path, size, and SHA-256.
class ScoreExportRunner : public QObject
{
    Q_OBJECT

public:
    explicit ScoreExportRunner(QDir repository, QObject *parent = nullptr);
    ~ScoreExportRunner() override;

    /// Return true only after one asynchronous attempt has taken ownership of the terminal
    /// signal. Busy and preflight rejections are synchronous, fill `error` when provided, and
    /// emit nothing, so they cannot suppress or impersonate an already-running attempt.
    bool startJava(const QString &input, int sheet, const QString &output,
                   ScoreExportResult *error = nullptr);
    void cancel();
    bool isRunning() const;

    void setJavaHome(const QString &path) { javaHome_ = path; }
    QString javaHome() const { return javaHome_; }
    static QString findJavaHome(const QDir &repository);

signals:
    void exportFinished(omrscope::ScoreExportResult result);

private:
    void consumeStdout();
    void consumeLine(const QString &line);
    void appendDiagnostic(const QString &text);
    void forceFailure(const QString &code, const QString &message);
    /// Stop the Gradle launcher and, where supported, every process it started.
    ///
    /// JavaExec outlives its Gradle launcher unless both share a killable process group. Do not
    /// emit the accepted attempt's terminal result before this has been requested; callers may
    /// then safely retire their per-attempt output directory from exportFinished.
    void stopProcessTree();
    void processFinished(int exitCode, QProcess::ExitStatus status);
    void emitFailure(const QString &code, const QString &message);
    void emitOnce(ScoreExportResult result);
    bool rejectStart(int sheet, const QString &code, const QString &message,
                     ScoreExportResult *error) const;
    std::optional<ScoreExportResult> validateArtifact(ScoreExportResult result,
                                                      QString *errorCode,
                                                      QString *errorMessage) const;
    void clearProcess();

    QDir repository_;
    QString javaHome_;
    QProcess *process_ = nullptr;
    QTimer timeout_;
    QByteArray pendingStdout_;
    QString diagnostics_;
    QString expectedOutput_;
    int expectedSheet_ = 0;
    std::optional<ScoreExportResult> marker_;
    QString forcedErrorCode_;
    QString forcedMessage_;
    bool terminalEmitted_ = false;
};

} // namespace omrscope

Q_DECLARE_METATYPE(omrscope::ScoreExportResult)
