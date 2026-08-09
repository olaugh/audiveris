// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include <QObject>
#include <QProcess>
#include <QString>
#include <QStringList>
#include <QTimer>

namespace omrscope {

/// One local score-engraving request.
///
/// `outputDirectory` must already exist and be owned by the caller (normally
/// a QTemporaryDir).  The renderer creates one uniquely-named child directory
/// within it, so that it never needs to overwrite a caller's files.
struct ScoreRenderRequest {
    QString inputPath;
    QString outputDirectory;
    int timeoutMs = 120'000;
};

/// The terminal outcome of a ScoreRenderer request.
struct ScoreRenderResult {
    QString renderer;
    QString rendererVersion;
    QString rendererExecutable;
    QStringList pagePaths;
    QString error;
    bool cancelled = false;
    bool timedOut = false;

    bool succeeded() const { return error.isEmpty() && !cancelled && !timedOut; }
};

/// Render MusicXML to SVG through a locally-installed Verovio executable.
///
/// This is deliberately independent of either recognition engine: Java and
/// Rust can hand the same exported .musicxml, .mxl, or conventional .xml file
/// to it.  It invokes a program directly through QProcess (never a shell) and
/// neither downloads nor installs a renderer.  An explicit OMRSCOPE_VEROVIO
/// takes precedence over PATH; when it is set but invalid it fails closed
/// rather than silently using another version from PATH.
class ScoreRenderer final : public QObject
{
    Q_OBJECT

public:
    explicit ScoreRenderer(QObject *parent = nullptr);

    /// Start one request without blocking the Qt event loop.
    ///
    /// Preflight failures, including an attempt while another request is
    /// active, are returned synchronously through false / `error`. A request
    /// that returns true has exactly one asynchronous finished signal.
    bool start(const ScoreRenderRequest &request, QString *error = nullptr);

    /// Stop the active renderer process.  Completion is reported by finished
    /// with `cancelled` set, once its process has exited.
    void cancel();

    bool isRunning() const;

    /// Resolve an installed executable without starting it.  This honours
    /// OMRSCOPE_VEROVIO before PATH, just like start().
    static QString findVerovioExecutable();

signals:
    void finished(const omrscope::ScoreRenderResult &result);

private:
    enum class Phase { Idle, Version, Render };

    static bool reject(QString error, QString *destination);
    void beginVersionProbe();
    void beginRender();
    void launch(const QStringList &arguments);
    void versionFinished(int exitCode, QProcess::ExitStatus exitStatus);
    void renderFinished(int exitCode, QProcess::ExitStatus exitStatus);
    void processError(QProcess::ProcessError error);
    void timeout();
    void stopProcess();
    bool collectPages(QString *error);
    void complete(ScoreRenderResult result);
    QString processDiagnostics() const;

    ScoreRenderRequest request_;
    ScoreRenderResult result_;
    QString inputCanonicalPath_;
    QString stagingDirectory_;
    QProcess *process_ = nullptr;
    QTimer *timeoutTimer_ = nullptr;
    Phase phase_ = Phase::Idle;
    bool active_ = false;
    bool cancelRequested_ = false;
    bool timeoutRequested_ = false;
};

} // namespace omrscope
