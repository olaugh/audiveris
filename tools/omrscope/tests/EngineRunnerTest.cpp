// SPDX-License-Identifier: AGPL-3.0-or-later
#include "EngineRunner.h"

#include <QCoreApplication>
#include <QEventLoop>
#include <QFile>
#include <QTimer>
#include <QTemporaryDir>
#include <QTextStream>

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

bool installProducer(const QString &repository)
{
    QDir root(repository);
    if (!root.mkpath(QStringLiteral("rust/target/release"))) {
        return false;
    }
    return installExecutable(root.filePath(
        QStringLiteral("rust/target/release/audiveris-cli")));
}

struct Observation {
    int completed = 0;
    int failed = 0;
    bool terminal = false;
    bool success = false;
    bool cancelled = false;
    QString message;
};

int fakeProducer(const QString &mode)
{
    if (mode == QLatin1String("stderr-exit")) {
        QTextStream(stderr) << "synthetic Gradle failure\n";
        return 9;
    }
    QTextStream out(stdout);
    const auto line = [&](const QString &text) { out << text << '\n'; out.flush(); };
    line(QStringLiteral("@omrscope {\"stream_schema\":1,\"engine\":\"rust\",\"event\":\"run_started\",\"sequence\":1}"));
    if (mode == QLatin1String("skip")) {
        line(QStringLiteral("@omrscope {\"stream_schema\":1,\"engine\":\"rust\",\"event\":\"stage_started\",\"sequence\":2,\"stage\":\"HEADS\"}"));
        return 0;
    }
    if (mode == QLatin1String("duplicate-sequence")) {
        line(QStringLiteral("@omrscope {\"stream_schema\":1,\"engine\":\"rust\",\"event\":\"stage_started\",\"sequence\":1,\"stage\":\"GRID\"}"));
        return 0;
    }
    line(QStringLiteral("@omrscope {\"stream_schema\":1,\"engine\":\"rust\",\"event\":\"stage_started\",\"sequence\":2,\"stage\":\"GRID\"}"));
    const QString payloadStage = mode == QLatin1String("mismatch")
        ? QStringLiteral("HEADERS") : QStringLiteral("GRID");
    line(QStringLiteral("{\"schema\":1,\"producer\":{\"stage\":\"%1\"},\"image\":{\"width\":1,\"height\":1}}")
             .arg(payloadStage));
    line(QStringLiteral("@omrscope {\"stream_schema\":1,\"engine\":\"rust\",\"event\":\"stage_completed\",\"sequence\":3,\"stage\":\"GRID\",\"elapsed_ms\":1}"));
    if (mode == QLatin1String("mismatch")) {
        return 0;
    }
    line(QStringLiteral("@omrscope {\"stream_schema\":1,\"engine\":\"rust\",\"event\":\"run_finished\",\"sequence\":4,\"success\":true}"));
    return mode == QLatin1String("bad-exit") ? 7 : 0;
}

} // namespace

int main(int argc, char **argv)
{
    QCoreApplication application(argc, argv);
    const QString producerMode = qEnvironmentVariable("OMRSCOPE_TEST_PRODUCER_MODE");
    if (!producerMode.isEmpty()) {
        return fakeProducer(producerMode);
    }
    QTemporaryDir repository;
    if (!repository.isValid()) {
        return 1;
    }

    omrscope::EngineRunner runner(QDir(repository.path()));
    int terminals = 0;
    bool allFailedActionably = true;
    QObject::connect(
        &runner, &omrscope::EngineRunner::streamFinished,
        [&](omrscope::Engine engine, bool success, bool cancelled, const QString &message) {
            ++terminals;
            allFailedActionably = allFailedActionably && engine == omrscope::Engine::Rust
                && !success && !cancelled && message.contains(QStringLiteral("no release binary"));
        });

    runner.startRust(QStringLiteral("missing.png"), 1, QStringLiteral("GRID"));
    runner.startRust(QStringLiteral("missing.png"), 1, QStringLiteral("GRID"));

    bool ok = true;
    ok &= check(terminals == 2,
                "preflight state resets so a second Run reports its own terminal failure");
    ok &= check(allFailedActionably,
                "pre-stage failures propagate an actionable run-level message");

    ok &= check(installProducer(repository.path()),
                "self-executable fake producer is installed");

    qputenv("OMRSCOPE_TEST_PRODUCER_MODE", QByteArrayLiteral("bad-exit"));
    const omrscope::EngineResult blockingRust = runner.runRust(
        QStringLiteral("ignored.png"), 1, QStringLiteral("GRID"));
    qunsetenv("OMRSCOPE_TEST_PRODUCER_MODE");
    ok &= check(!blockingRust.ran
                    && blockingRust.error.contains(QStringLiteral("exited with code 7")),
                "blocking Rust runner propagates a nonzero process exit");

    ok &= check(installExecutable(QDir(repository.path()).filePath(QStringLiteral("gradlew"))),
                "self-executable fake Gradle launcher is installed");
    runner.setJavaHome(QStringLiteral("synthetic-java-home"));
    qputenv("OMRSCOPE_TEST_PRODUCER_MODE", QByteArrayLiteral("stderr-exit"));
    const omrscope::EngineResult blockingJava = runner.runJava(
        QStringLiteral("ignored.png"), 1, QStringLiteral("HEADS"));
    qunsetenv("OMRSCOPE_TEST_PRODUCER_MODE");
    ok &= check(!blockingJava.ran
                    && blockingJava.error.contains(QStringLiteral("Gradle exited with code 9"))
                    && blockingJava.error.contains(QStringLiteral("synthetic Gradle failure")),
                "blocking Java runner propagates child exit and stderr diagnostics");

    auto observe = [&](const QString &through, const QByteArray &mode) {
        Observation observation;
        QEventLoop loop;
        const auto eventConnection = QObject::connect(
            &runner, &omrscope::EngineRunner::stageEvent,
            [&](omrscope::Engine, omrscope::StreamEvent event, omrscope::EngineResult) {
                observation.completed += event.event == QLatin1String("stage_completed") ? 1 : 0;
                observation.failed += event.event == QLatin1String("stage_failed") ? 1 : 0;
            });
        const auto terminalConnection = QObject::connect(
            &runner, &omrscope::EngineRunner::streamFinished,
            [&](omrscope::Engine, bool success, bool cancelled, const QString &message) {
                observation.terminal = true;
                observation.success = success;
                observation.cancelled = cancelled;
                observation.message = message;
                loop.quit();
            });
        qputenv("OMRSCOPE_TEST_PRODUCER_MODE", mode);
        runner.startRust(QStringLiteral("ignored.png"), 1, through);
        if (!observation.terminal) {
            QTimer::singleShot(5000, &loop, &QEventLoop::quit);
            loop.exec();
        }
        qunsetenv("OMRSCOPE_TEST_PRODUCER_MODE");
        QObject::disconnect(eventConnection);
        QObject::disconnect(terminalConnection);
        return observation;
    };

    const Observation valid = observe(QStringLiteral("GRID"), QByteArrayLiteral("valid"));
    ok &= check(valid.terminal && valid.success && valid.completed == 1,
                "valid framed stage and successful terminal marker complete the run");

    const Observation skippedPlan = observe(QStringLiteral("HEADS"), QByteArrayLiteral("skip"));
    ok &= check(skippedPlan.terminal && !skippedPlan.success
                    && skippedPlan.message.contains(QStringLiteral("GRID was expected")),
                "producer cannot skip the exact GRID-through-target stage plan");

    const Observation duplicateSequence = observe(
        QStringLiteral("GRID"), QByteArrayLiteral("duplicate-sequence"));
    ok &= check(duplicateSequence.terminal && !duplicateSequence.success
                    && duplicateSequence.message.contains(QStringLiteral("non-monotonic")),
                "marker sequences must be positive and strictly increasing");

    const Observation badExit = observe(QStringLiteral("GRID"), QByteArrayLiteral("bad-exit"));
    ok &= check(badExit.terminal && !badExit.success && badExit.completed == 1
                    && badExit.message.contains(QStringLiteral("code 7")),
                "nonzero exit remains a run-level failure after a completed snapshot");

    const Observation mismatch = observe(QStringLiteral("GRID"), QByteArrayLiteral("mismatch"));
    ok &= check(mismatch.terminal && !mismatch.success && mismatch.failed == 1
                    && mismatch.message.contains(QStringLiteral("declares stage HEADERS")),
                "payload stage mismatch fails the active stage and run");

    return ok ? 0 : 1;
}
