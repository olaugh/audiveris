// SPDX-License-Identifier: AGPL-3.0-or-later
#include "EngineRunner.h"

#include "Parsers.h"

#include <QElapsedTimer>
#include <QFileInfo>
#include <QProcess>
#include <QProcessEnvironment>
#include <QPointer>
#include <QTimer>

namespace omrscope {

namespace {

/// Java probes take a long time; a short timeout would look like a failure.
constexpr int kJavaTimeoutMs = 15 * 60 * 1000;
constexpr int kRustTimeoutMs = 5 * 60 * 1000;
constexpr qsizetype kMaxDiagnosticChars = 64 * 1024;

QString engineName(Engine engine)
{
    return engine == Engine::Rust ? QStringLiteral("rust") : QStringLiteral("java");
}

QStringList streamPlan(const QString &through)
{
    static const QStringList stages{
        QStringLiteral("GRID"), QStringLiteral("HEADERS"), QStringLiteral("STEM_SEEDS"),
        QStringLiteral("BEAMS"), QStringLiteral("LEDGERS"), QStringLiteral("HEADS"),
    };
    const int target = stages.indexOf(through);
    return target < 0 ? QStringList{} : stages.mid(0, target + 1);
}

} // namespace

EngineRunner::EngineRunner(QDir repository, QObject *parent)
    : QObject(parent)
    , repository_(std::move(repository))
    , javaHome_(findJavaHome())
{
}

QString EngineRunner::rustBinary() const
{
    return repository_.filePath(QStringLiteral("rust/target/release/audiveris-cli"));
}

QString EngineRunner::findJavaHome()
{
    // The JDK the oracles were generated with is a sibling of the Audiveris
    // checkout rather than a system JVM, which is why `/usr/libexec/java_home`
    // does not see it. Honour an explicit JAVA_HOME first.
    const QString fromEnvironment = qEnvironmentVariable("JAVA_HOME");
    if (!fromEnvironment.isEmpty() && QFileInfo::exists(fromEnvironment + "/bin/java")) {
        return fromEnvironment;
    }
    const QStringList candidates = {
        QStringLiteral("/Users/john/sources/jul10-charter/omr/tools/jdk25/Contents/Home"),
    };
    for (const QString &candidate : candidates) {
        if (QFileInfo::exists(candidate + "/bin/java")) {
            return candidate;
        }
    }
    return {};
}

EngineResult EngineRunner::runRust(const QString &input, int sheet, const QString &step)
{
    EngineResult result;
    result.engine = QStringLiteral("rust");
    result.timingNote = QStringLiteral("process wall clock; startup is a few ms");

    const QString binary = rustBinary();
    if (!QFileInfo::exists(binary)) {
        result.error = QStringLiteral("no release binary at %1\n\nBuild it:\n"
                                      "  cd rust && cargo build --release -p audiveris-cli")
                           .arg(binary);
        return result;
    }

    QProcess process;
    process.setWorkingDirectory(repository_.absolutePath());
    const QStringList arguments{QStringLiteral("-batch"),
                                QStringLiteral("-step"),
                                step,
                                QStringLiteral("-json"),
                                input,
                                QStringLiteral("-sheets"),
                                QString::number(sheet)};

    QElapsedTimer timer;
    timer.start();
    process.start(binary, arguments);
    if (!process.waitForStarted()) {
        result.error = QStringLiteral("could not start Rust process: %1").arg(process.errorString());
        return result;
    }
    if (!process.waitForFinished(kRustTimeoutMs)) {
        const QString reason = process.errorString();
        process.kill();
        process.waitForFinished(5000);
        const QString diagnostics = QString::fromUtf8(process.readAllStandardError()).trimmed();
        result.error = QStringLiteral("Rust process did not finish: %1").arg(reason);
        if (!diagnostics.isEmpty()) {
            result.error += QStringLiteral(": %1").arg(diagnostics);
        }
        return result;
    }
    result.millis = static_cast<double>(timer.nsecsElapsed()) / 1e6;

    const QString output = QString::fromUtf8(process.readAllStandardOutput());
    const QString diagnostics = QString::fromUtf8(process.readAllStandardError()).trimmed();
    if (process.exitStatus() != QProcess::NormalExit || process.exitCode() != 0) {
        result.error = process.exitStatus() == QProcess::CrashExit
            ? QStringLiteral("Rust process crashed")
            : QStringLiteral("Rust process exited with code %1").arg(process.exitCode());
        if (!diagnostics.isEmpty()) {
            result.error += QStringLiteral(": %1").arg(diagnostics);
        }
        return result;
    }
    if (output.trimmed().isEmpty()) {
        result.error = diagnostics;
        if (result.error.isEmpty()) {
            result.error = QStringLiteral("no output");
        }
        return result;
    }

    EngineResult parsed = parseRustJson(output);
    parsed.millis = result.millis;
    parsed.timingNote = result.timingNote;
    if (!parsed.ran && !diagnostics.isEmpty()) {
        parsed.error += QStringLiteral(": %1").arg(diagnostics);
    }
    return parsed;
}

EngineResult EngineRunner::runJava(const QString &input, int sheet, const QString &step)
{
    EngineResult result;
    result.engine = QStringLiteral("java");
    result.timingNote = QStringLiteral("in-process reachStep only; excludes Gradle and JVM startup");

    if (javaHome_.isEmpty()) {
        result.error = QStringLiteral("no JDK found. Set JAVA_HOME to a JDK 25.");
        return result;
    }

    QProcessEnvironment environment = QProcessEnvironment::systemEnvironment();
    environment.insert(QStringLiteral("JAVA_HOME"), javaHome_);
    // A proxy banner on stdout corrupts every parsed line. This has bitten the
    // oracles before, so it is cleared here rather than assumed absent.
    environment.remove(QStringLiteral("JAVA_TOOL_OPTIONS"));

    QProcess process;
    process.setWorkingDirectory(repository_.absolutePath());
    process.setProcessEnvironment(environment);

    const QString target = QStringLiteral("%1:%2:%3").arg(input).arg(sheet).arg(step);
    const QStringList arguments{
        QStringLiteral("--no-daemon"),
        QStringLiteral("-q"),
        QStringLiteral("-I"),
        QStringLiteral("rust/oracle/java/staff-impacts.init.gradle"),
        QStringLiteral(":app:sigProbe"),
        QStringLiteral("-PsigTargets=%1").arg(target),
    };

    process.start(repository_.filePath(QStringLiteral("gradlew")), arguments);
    if (!process.waitForStarted()) {
        result.error = QStringLiteral("could not start Gradle: %1").arg(process.errorString());
        return result;
    }
    if (!process.waitForFinished(kJavaTimeoutMs)) {
        const QString reason = process.errorString();
        process.kill();
        process.waitForFinished(5000);
        const QString diagnostics = QString::fromUtf8(process.readAllStandardError()).trimmed();
        result.error = QStringLiteral("Gradle did not finish: %1").arg(reason);
        if (!diagnostics.isEmpty()) {
            result.error += QStringLiteral(": %1").arg(diagnostics);
        }
        return result;
    }

    const QString output = QString::fromUtf8(process.readAllStandardOutput());
    const QString diagnostics = QString::fromUtf8(process.readAllStandardError()).trimmed();
    if (process.exitStatus() != QProcess::NormalExit || process.exitCode() != 0) {
        result.error = process.exitStatus() == QProcess::CrashExit
            ? QStringLiteral("Gradle process crashed")
            : QStringLiteral("Gradle exited with code %1").arg(process.exitCode());
        if (!diagnostics.isEmpty()) {
            result.error += QStringLiteral(": %1").arg(diagnostics);
        }
        return result;
    }
    EngineResult parsed = parseSigProbe(output);
    parsed.timingNote = result.timingNote;
    if (!parsed.ran && !diagnostics.isEmpty()) {
        parsed.error += QStringLiteral(": %1").arg(diagnostics);
    }
    if (!parsed.ran && parsed.error.isEmpty()) {
        parsed.error = QStringLiteral("no records; is the Gradle build working?");
    }
    return parsed;
}

EngineRunner::ActiveRun &EngineRunner::active(Engine engine)
{
    return engine == Engine::Rust ? rustRun_ : javaRun_;
}

const EngineRunner::ActiveRun &EngineRunner::active(Engine engine) const
{
    return engine == Engine::Rust ? rustRun_ : javaRun_;
}

bool EngineRunner::isRunning(Engine engine) const
{
    const QProcess *process = active(engine).process;
    return process && process->state() != QProcess::NotRunning;
}

void EngineRunner::startRust(const QString &input, int sheet, const QString &through)
{
    if (isRunning(Engine::Rust)) {
        return;
    }
    reset(Engine::Rust);
    active(Engine::Rust).expectedStages = streamPlan(through);
    if (active(Engine::Rust).expectedStages.isEmpty()) {
        failStart(Engine::Rust, QStringLiteral("unsupported streaming target %1").arg(through));
        return;
    }
    const QString binary = rustBinary();
    if (!QFileInfo::exists(binary)) {
        failStart(Engine::Rust, QStringLiteral("no release binary at %1\n\nBuild it:\n"
                                               "  cd rust && cargo build --release -p audiveris-cli")
                                     .arg(binary));
        return;
    }
    start(Engine::Rust, binary,
          {QStringLiteral("-batch"), QStringLiteral("-step"), through,
           QStringLiteral("-json"), QStringLiteral("-stream-json"), input,
           QStringLiteral("-sheets"), QString::number(sheet)},
          repository_.absolutePath());
}

void EngineRunner::startJava(const QString &input, int sheet, const QString &through)
{
    if (isRunning(Engine::Java)) {
        return;
    }
    reset(Engine::Java);
    active(Engine::Java).expectedStages = streamPlan(through);
    if (active(Engine::Java).expectedStages.isEmpty()) {
        failStart(Engine::Java, QStringLiteral("unsupported streaming target %1").arg(through));
        return;
    }
    if (javaHome_.isEmpty()) {
        failStart(Engine::Java, QStringLiteral("no JDK found. Set JAVA_HOME to a JDK 25."));
        return;
    }
    QProcessEnvironment environment = QProcessEnvironment::systemEnvironment();
    environment.insert(QStringLiteral("JAVA_HOME"), javaHome_);
    environment.remove(QStringLiteral("JAVA_TOOL_OPTIONS"));
    const QString target = QStringLiteral("%1:%2:%3").arg(input).arg(sheet).arg(through);
    start(Engine::Java, repository_.filePath(QStringLiteral("gradlew")),
          {QStringLiteral("--no-daemon"), QStringLiteral("-q"),
           QStringLiteral("-I"), QStringLiteral("rust/oracle/java/staff-impacts.init.gradle"),
           QStringLiteral(":app:sigStreamProbe"),
           QStringLiteral("-PsigStreamTargets=%1").arg(target)},
          repository_.absolutePath(), environment);
}

void EngineRunner::start(Engine engine, const QString &program, const QStringList &arguments,
                         const QString &workingDirectory,
                         const QProcessEnvironment &environment)
{
    ActiveRun &run = active(engine);
    auto *process = new QProcess(this);
    run.process = process;
    process->setWorkingDirectory(workingDirectory);
    process->setProcessChannelMode(QProcess::SeparateChannels);
    if (!environment.isEmpty()) {
        process->setProcessEnvironment(environment);
    }
    connect(process, &QProcess::readyReadStandardOutput, this,
            [this, engine] { consumeStdout(engine); });
    connect(process, &QProcess::readyReadStandardError, this, [this, engine] {
        ActiveRun &activeRun = active(engine);
        if (activeRun.process) {
            appendDiagnostic(activeRun,
                             QString::fromUtf8(activeRun.process->readAllStandardError()));
        }
    });
    connect(process, &QProcess::errorOccurred, this,
            [this, engine](QProcess::ProcessError error) {
                if (error == QProcess::FailedToStart) {
                    ActiveRun &activeRun = active(engine);
                    const QString reason = activeRun.process ? activeRun.process->errorString()
                                                             : QStringLiteral("failed to start");
                    failStart(engine, reason);
                }
            });
    connect(process, qOverload<int, QProcess::ExitStatus>(&QProcess::finished), this,
            [this, engine](int code, QProcess::ExitStatus status) { finish(engine, code, status); });
    process->start(program, arguments);
}

void EngineRunner::reset(Engine engine)
{
    ActiveRun &run = active(engine);
    if (run.process) {
        run.process->deleteLater();
    }
    run = ActiveRun{};
}

void EngineRunner::appendDiagnostic(ActiveRun &run, const QString &text)
{
    if (text.isEmpty()) {
        return;
    }
    run.diagnostics += text;
    if (run.diagnostics.size() > kMaxDiagnosticChars) {
        run.diagnostics.remove(0, run.diagnostics.size() - kMaxDiagnosticChars);
    }
}

void EngineRunner::consumeStdout(Engine engine)
{
    ActiveRun &run = active(engine);
    if (!run.process) {
        return;
    }
    run.pendingStdout += run.process->readAllStandardOutput();
    for (const QString &line : takeCompleteLines(run.pendingStdout)) {
        consumeLine(engine, line);
    }
}

void EngineRunner::consumeLine(Engine engine, const QString &line)
{
    ActiveRun &run = active(engine);
    static const QString prefix = QStringLiteral("@omrscope ");
    if (!line.startsWith(prefix)) {
        if (!run.activeStage.isEmpty()) {
            run.activePayload += line;
            run.activePayload += QLatin1Char('\n');
        } else {
            appendDiagnostic(run, line + QLatin1Char('\n'));
        }
        return;
    }

    QString markerError;
    const auto marker = parseStreamMarker(line.mid(prefix.size()), engine, &markerError);
    if (!marker) {
        protocolFailure(engine, markerError, run.activeStage);
        return;
    }

    if (marker->sequence <= 0 || marker->sequence <= run.lastSequence) {
        protocolFailure(engine,
                        QStringLiteral("non-monotonic stream sequence %1 after %2")
                            .arg(marker->sequence)
                            .arg(run.lastSequence),
                        run.activeStage);
        return;
    }
    run.lastSequence = marker->sequence;

    const bool known = marker->event == QLatin1String("run_started")
        || marker->event == QLatin1String("stage_started")
        || marker->event == QLatin1String("stage_completed")
        || marker->event == QLatin1String("stage_failed")
        || marker->event == QLatin1String("run_finished");
    if (!known) {
        protocolFailure(engine, QStringLiteral("unknown stream event %1").arg(marker->event),
                        run.activeStage);
        return;
    }
    if (run.sawRunFinished) {
        protocolFailure(engine, QStringLiteral("stream event arrived after run_finished"),
                        run.activeStage);
        return;
    }
    if (run.sawStageFailure && marker->event != QLatin1String("run_finished")) {
        protocolFailure(engine,
                        QStringLiteral("only run_finished may follow a failed stage"),
                        run.activeStage);
        return;
    }

    if (marker->event == QLatin1String("run_started")) {
        if (run.sawRunStarted || !run.activeStage.isEmpty() || !run.startedStages.isEmpty()) {
            protocolFailure(engine, QStringLiteral("conflicting second run_started marker"),
                            run.activeStage);
            return;
        }
        run.sawRunStarted = true;
        emit stageEvent(engine, *marker, EngineResult{});
        return;
    }

    if (marker->event == QLatin1String("stage_started")) {
        if (!run.sawRunStarted) {
            protocolFailure(engine, QStringLiteral("stage_started arrived before run_started"));
            return;
        }
        if (!run.activeStage.isEmpty()) {
            protocolFailure(engine,
                            QStringLiteral("stage %1 started while %2 is active")
                                .arg(marker->stage, run.activeStage),
                            run.activeStage);
            return;
        }
        if (run.nextStageIndex >= run.expectedStages.size()
            || run.expectedStages[run.nextStageIndex] != marker->stage) {
            const QString expected = run.nextStageIndex < run.expectedStages.size()
                ? run.expectedStages[run.nextStageIndex] : QStringLiteral("<end of plan>");
            protocolFailure(engine,
                            QStringLiteral("stage %1 arrived where %2 was expected")
                                .arg(marker->stage, expected));
            return;
        }
        if (run.startedStages.contains(marker->stage)
            || run.completedStages.contains(marker->stage)) {
            protocolFailure(engine,
                            QStringLiteral("conflicting second start for stage %1")
                                .arg(marker->stage));
            return;
        }
        run.startedStages.insert(marker->stage);
        run.activeStage = marker->stage;
        run.activePayload.clear();
        emit stageEvent(engine, *marker, EngineResult{});
        return;
    }
    if (marker->event == QLatin1String("stage_completed")) {
        if (run.activeStage != marker->stage) {
            protocolFailure(engine,
                            QStringLiteral("completion for %1 does not match active stage %2")
                                .arg(marker->stage, run.activeStage),
                            run.activeStage);
            return;
        }
        EngineResult result = engine == Engine::Rust ? parseRustJson(run.activePayload)
                                                      : parseSigProbe(run.activePayload);
        if (!result.error.isEmpty()) {
            protocolFailure(engine,
                            QStringLiteral("%1 payload failed to parse: %2")
                                .arg(marker->stage, result.error),
                            run.activeStage);
            return;
        }
        if (result.declaredStage.isEmpty() || result.declaredStage != marker->stage) {
            protocolFailure(engine,
                            QStringLiteral("payload declares stage %1 inside %2 frame")
                                .arg(result.declaredStage.isEmpty()
                                         ? QStringLiteral("<missing>") : result.declaredStage,
                                     marker->stage),
                            run.activeStage);
            return;
        }
        if (marker->elapsedMillis) {
            result.millis = *marker->elapsedMillis;
        }
        if (result.timingNote.isEmpty()) {
            result.timingNote = engine == Engine::Rust
                ? QStringLiteral("stage-boundary process time")
                : QStringLiteral("in-process stage time");
        }
        run.completedStages.insert(marker->stage);
        ++run.nextStageIndex;
        run.activeStage.clear();
        run.activePayload.clear();
        emit stageEvent(engine, *marker, result);
        return;
    }
    if (marker->event == QLatin1String("stage_failed")) {
        if (run.activeStage != marker->stage) {
            protocolFailure(engine,
                            QStringLiteral("failure for %1 does not match active stage %2")
                                .arg(marker->stage, run.activeStage),
                            run.activeStage);
            return;
        }
        StreamEvent failed = *marker;
        if (failed.message.isEmpty()) {
            failed.message = QStringLiteral("recognition failed");
        }
        EngineResult result;
        result.engine = engineName(engine);
        result.declaredStage = marker->stage;
        result.error = failed.message;
        result.raw = run.activePayload;
        emit stageEvent(engine, failed, result);
        run.sawStageFailure = true;
        ++run.nextStageIndex;
        run.activeStage.clear();
        run.activePayload.clear();
        return;
    }
    // run_finished is a mandatory terminal protocol verdict, independent of
    // the child process exit code.
    if (!run.sawRunStarted || !run.activeStage.isEmpty() || !marker->success) {
        protocolFailure(engine, QStringLiteral("invalid run_finished marker"), run.activeStage);
        return;
    }
    run.sawRunFinished = true;
    run.runFinishedSuccess = *marker->success;
    if (run.runFinishedSuccess
        && (run.sawStageFailure || run.nextStageIndex != run.expectedStages.size())) {
        protocolFailure(engine,
                        run.sawStageFailure
                            ? QStringLiteral("run_finished success followed a failed stage")
                            : QStringLiteral("run_finished success before the expected stage plan completed"));
        return;
    }
    if (!run.runFinishedSuccess) {
        run.protocolFailed = true;
        run.protocolMessage = marker->message.isEmpty()
            ? QStringLiteral("producer reported an unsuccessful run") : marker->message;
    }
    emit stageEvent(engine, *marker, EngineResult{});
}

void EngineRunner::protocolFailure(Engine engine, const QString &message,
                                   const QString &activeStage)
{
    ActiveRun &run = active(engine);
    if (run.protocolFailed) {
        return;
    }
    run.protocolFailed = true;
    run.protocolMessage = message;
    if (!activeStage.isEmpty() && !run.completedStages.contains(activeStage)) {
        StreamEvent failed;
        failed.event = QStringLiteral("stage_failed");
        failed.stage = activeStage;
        failed.sequence = run.lastSequence;
        failed.message = message;
        EngineResult result;
        result.engine = engineName(engine);
        result.declaredStage = activeStage;
        result.error = message;
        result.raw = run.activePayload;
        emit stageEvent(engine, failed, result);
    }
    run.activeStage.clear();
    run.activePayload.clear();
    if (run.process && run.process->state() != QProcess::NotRunning) {
        QPointer<QProcess> process(run.process);
        process->terminate();
        QTimer::singleShot(3000, this, [process] {
            if (process && process->state() != QProcess::NotRunning) {
                process->kill();
            }
        });
    }
}

void EngineRunner::cancel(Engine engine)
{
    ActiveRun &run = active(engine);
    if (!run.process || run.process->state() == QProcess::NotRunning) {
        return;
    }
    run.cancelled = true;
    QPointer<QProcess> process(run.process);
    run.process->terminate();
    QTimer::singleShot(3000, this, [process] {
        if (process && process->state() != QProcess::NotRunning) {
            process->kill();
        }
    });
}

void EngineRunner::finish(Engine engine, int exitCode, QProcess::ExitStatus status)
{
    ActiveRun &run = active(engine);
    if (run.terminalEmitted) {
        return;
    }
    consumeStdout(engine);
    if (run.process) {
        appendDiagnostic(run, QString::fromUtf8(run.process->readAllStandardError()));
    }
    if (!run.pendingStdout.isEmpty()) {
        consumeLine(engine, QString::fromUtf8(run.pendingStdout));
        run.pendingStdout.clear();
    }
    if (!run.activeStage.isEmpty() && !run.protocolFailed) {
        StreamEvent terminal;
        terminal.stage = run.activeStage;
        terminal.event = run.cancelled ? QStringLiteral("stage_cancelled")
                                       : QStringLiteral("stage_failed");
        terminal.message = run.cancelled ? QStringLiteral("cancelled")
                                         : QStringLiteral("process exited before completing the stage");
        EngineResult result;
        result.engine = engineName(engine);
        result.declaredStage = terminal.stage;
        result.error = terminal.message;
        result.raw = run.activePayload;
        emit stageEvent(engine, terminal, result);
        run.activeStage.clear();
        run.activePayload.clear();
    }
    run.terminalEmitted = true;
    bool success = false;
    QString message;
    if (run.cancelled) {
        message = QStringLiteral("cancelled");
    } else if (run.protocolFailed) {
        message = run.protocolMessage;
    } else if (status != QProcess::NormalExit || exitCode != 0) {
        message = status == QProcess::CrashExit
            ? QStringLiteral("%1 process crashed").arg(engineName(engine))
            : QStringLiteral("%1 process exited with code %2").arg(engineName(engine)).arg(exitCode);
    } else if (!run.sawRunFinished) {
        message = QStringLiteral("%1 process exited without run_finished").arg(engineName(engine));
    } else if (!run.runFinishedSuccess) {
        message = QStringLiteral("%1 producer reported an unsuccessful run").arg(engineName(engine));
    } else {
        success = true;
        message = QStringLiteral("complete");
    }
    const QString diagnostics = run.diagnostics.trimmed();
    if (!success && !diagnostics.isEmpty()) {
        message += QStringLiteral(": %1").arg(diagnostics.right(1000));
    }
    QProcess *process = run.process;
    run.process = nullptr;
    emit streamFinished(engine, success, run.cancelled, message);
    if (process) {
        process->deleteLater();
    }
}

void EngineRunner::failStart(Engine engine, const QString &message)
{
    ActiveRun &run = active(engine);
    if (run.terminalEmitted) {
        return;
    }
    run.terminalEmitted = true;
    StreamEvent failed;
    failed.event = QStringLiteral("stage_failed");
    failed.message = message;
    EngineResult result;
    result.engine = engineName(engine);
    result.error = message;
    emit stageEvent(engine, failed, result);
    emit streamFinished(engine, false, false, message);
    if (run.process) {
        run.process->deleteLater();
        run.process = nullptr;
    }
}

} // namespace omrscope
