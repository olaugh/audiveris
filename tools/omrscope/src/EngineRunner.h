// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include "Model.h"

#include <QDir>
#include <QObject>
#include <QProcess>
#include <QProcessEnvironment>
#include <QSet>
#include <QString>

namespace omrscope {

/// Runs the two engines over one sheet and reports what each made of it.
///
/// Both are run as child processes rather than linked: the Rust side is a
/// binary and the Java side needs a JVM, and shelling out is also what keeps
/// this tool honest -- it observes exactly what the engines emit to a caller,
/// with no privileged access to their internals.
class EngineRunner : public QObject
{
    Q_OBJECT

public:
    explicit EngineRunner(QDir repository, QObject *parent = nullptr);

    /// The Rust CLI, timed by process wall clock.
    ///
    /// Its startup is a few milliseconds, so the process time is a fair
    /// measure of recognition. If the release binary is absent the result
    /// carries the command that would build it rather than a bare failure.
    EngineResult runRust(const QString &input, int sheet, const QString &step);

    /// Java through Gradle, timed *inside* the probe.
    ///
    /// Wall clock here is dominated by Gradle and JVM startup -- tens of
    /// seconds against a few hundred milliseconds of actual work -- so the
    /// probe reports its own `reachStep` duration and that is what is shown.
    /// Comparing the two wall clocks would say nothing about either engine.
    EngineResult runJava(const QString &input, int sheet, const QString &step);

    /// Start both producers without blocking the event loop.  A producer
    /// writes marker lines around its unchanged stage payload; snapshots are
    /// emitted as each `stage_completed` marker arrives.
    void startRust(const QString &input, int sheet, const QString &through);
    void startJava(const QString &input, int sheet, const QString &through);
    void cancel(Engine engine);
    bool isRunning(Engine engine) const;

    void setJavaHome(const QString &path) { javaHome_ = path; }
    QString javaHome() const { return javaHome_; }

    /// Where the Rust release binary should be, whether or not it exists.
    QString rustBinary() const;

    static QString findJavaHome();

signals:
    /// `result` is meaningful for completed/failed events.  It is copied so
    /// the MainWindow can retain a completed stage after a later event.
    void stageEvent(omrscope::Engine engine, omrscope::StreamEvent event,
                    omrscope::EngineResult result);
    void streamFinished(omrscope::Engine engine, bool success, bool cancelled,
                        QString message);

private:
    struct ActiveRun {
        QProcess *process = nullptr;
        QByteArray pendingStdout;
        QString activeStage;
        QString activePayload;
        QString diagnostics;
        QSet<QString> startedStages;
        QSet<QString> completedStages;
        QStringList expectedStages;
        int nextStageIndex = 0;
        int lastSequence = 0;
        bool sawRunStarted = false;
        bool sawRunFinished = false;
        bool runFinishedSuccess = false;
        bool sawStageFailure = false;
        bool cancelled = false;
        bool protocolFailed = false;
        QString protocolMessage;
        bool terminalEmitted = false;
    };

    ActiveRun &active(Engine engine);
    const ActiveRun &active(Engine engine) const;
    void start(Engine engine, const QString &program, const QStringList &arguments,
               const QString &workingDirectory, const QProcessEnvironment &environment = {});
    void reset(Engine engine);
    void consumeStdout(Engine engine);
    void consumeLine(Engine engine, const QString &line);
    void appendDiagnostic(ActiveRun &run, const QString &text);
    void protocolFailure(Engine engine, const QString &message,
                         const QString &activeStage = {});
    void finish(Engine engine, int exitCode, QProcess::ExitStatus status);
    void failStart(Engine engine, const QString &message);

    QDir repository_;
    QString javaHome_;
    ActiveRun rustRun_;
    ActiveRun javaRun_;
};

} // namespace omrscope
