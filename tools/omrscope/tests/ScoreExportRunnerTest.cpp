// SPDX-License-Identifier: AGPL-3.0-or-later
#include "ScoreExportRunner.h"

#include <QCoreApplication>
#include <QCryptographicHash>
#include <QElapsedTimer>
#include <QEventLoop>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QTemporaryDir>
#include <QTextStream>
#include <QThread>
#include <QTimer>

#ifdef Q_OS_UNIX
#include <fcntl.h>
#include <sys/types.h>
#include <unistd.h>
#endif

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

QString argumentValue(const QStringList &arguments, const QString &property)
{
    const QString prefix = QStringLiteral("-P%1=").arg(property);
    for (const QString &argument : arguments) {
        if (argument.startsWith(prefix)) {
            return argument.mid(prefix.size());
        }
    }
    return {};
}

QByteArray fileDigest(const QString &path)
{
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly)) {
        return {};
    }
    QCryptographicHash hash(QCryptographicHash::Sha256);
    return hash.addData(&file) ? hash.result().toHex() : QByteArray{};
}

#ifdef Q_OS_UNIX
bool writeChildSentinel(const QByteArray &path)
{
    const int descriptor = ::open(path.constData(), O_WRONLY | O_CREAT | O_TRUNC, 0600);
    if (descriptor < 0) {
        return false;
    }
    constexpr char contents[] = "child survived\n";
    const ssize_t written = ::write(descriptor, contents, sizeof(contents) - 1);
    ::close(descriptor);
    return written == static_cast<ssize_t>(sizeof(contents) - 1);
}
#endif

int fakeGradle(const QString &mode, const QStringList &arguments)
{
    const QString output = argumentValue(arguments, QStringLiteral("musicXmlExportOutput"));
    const int sheet =
        argumentValue(arguments, QStringLiteral("musicXmlExportSheet")).toInt();
    if (mode == QLatin1String("delay")) {
        QThread::msleep(250);
    }
    if (mode == QLatin1String("failure")) {
        const QJsonObject marker{
            {QStringLiteral("schema"), 1},
            {QStringLiteral("engine"), QStringLiteral("java")},
            {QStringLiteral("event"), QStringLiteral("export_finished")},
            {QStringLiteral("stage"), QStringLiteral("PAGE")},
            {QStringLiteral("success"), false},
            {QStringLiteral("sheet"), sheet},
            {QStringLiteral("error_code"), QStringLiteral("page_failed")},
            {QStringLiteral("message"), QStringLiteral("synthetic PAGE failure")},
            {QStringLiteral("elapsed_ms"), 2.5},
        };
        QTextStream(stdout) << "@omrscope-export "
                            << QJsonDocument(marker).toJson(QJsonDocument::Compact) << '\n';
        return 2;
    }

#ifdef Q_OS_UNIX
    if (mode == QLatin1String("immediate-cancel-write")) {
        // If cancellation loses the fork-to-setsid race and kills only a nonexistent process
        // group, this launcher survives long enough to expose the miss.
        ::usleep(800 * 1000);
        return writeChildSentinel(
            QFile::encodeName(output + QStringLiteral(".launcher-survived"))) ? 0 : 11;
    }
    if (mode == QLatin1String("orphan-write")) {
        // Model Gradle's JavaExec child. It deliberately has no connection to the marker
        // protocol: when cancellation terminates only the wrapper, this child would otherwise
        // outlive the terminal signal and write after the caller tears down its attempt dir.
        const QByteArray ready = QFile::encodeName(output + QStringLiteral(".child-ready"));
        const QByteArray sentinel =
            QFile::encodeName(output + QStringLiteral(".child-survived"));
        const pid_t child = ::fork();
        if (child < 0) {
            return 9;
        }
        if (child == 0) {
            ::usleep(800 * 1000);
            writeChildSentinel(sentinel);
            _exit(0);
        }
        if (!writeChildSentinel(ready)) {
            return 10;
        }
        // Keep the launcher alive until the runner cancels it. The child must not be allowed to
        // make its delayed write after the runner observes this launcher's exit.
        ::usleep(3000 * 1000);
        return 0;
    }
#endif

    QFile artifact(output);
    if (!artifact.open(QIODevice::WriteOnly)) {
        return 8;
    }
    artifact.write("<score-partwise version=\"4.0\"/>\n");
    artifact.close();

    QString reportedPath = QFileInfo(output).canonicalFilePath();
    if (mode == QLatin1String("redirect")) {
        reportedPath = QFileInfo(output).absoluteDir().filePath(QStringLiteral("redirected.mxl"));
    }
    QByteArray digest = fileDigest(output);
    if (mode == QLatin1String("bad-hash")) {
        digest = QByteArray(64, '0');
    }
    const QJsonObject marker{
        {QStringLiteral("schema"), 1},
        {QStringLiteral("engine"), QStringLiteral("java")},
        {QStringLiteral("event"), QStringLiteral("export_finished")},
        {QStringLiteral("stage"), QStringLiteral("PAGE")},
        {QStringLiteral("success"), true},
        {QStringLiteral("sheet"), sheet},
        {QStringLiteral("format"), QStringLiteral("mxl")},
        {QStringLiteral("path"), reportedPath},
        {QStringLiteral("bytes"), QFileInfo(output).size()},
        {QStringLiteral("sha256"), QString::fromLatin1(digest)},
        {QStringLiteral("elapsed_ms"), 12.25},
    };
    QTextStream out(stdout);
    out << "synthetic Gradle diagnostic\n";
    const QString line = QStringLiteral("@omrscope-export ")
        + QString::fromUtf8(QJsonDocument(marker).toJson(QJsonDocument::Compact));
    out << line << '\n';
    if (mode == QLatin1String("duplicate")) {
        out << line << '\n';
    }
    out.flush();
    return mode == QLatin1String("bad-exit") ? 7 : 0;
}

struct Observation {
    int terminals = 0;
    bool started = false;
    omrscope::ScoreExportResult result;
};

Observation run(omrscope::ScoreExportRunner &runner, const QString &mode,
                const QString &input, const QString &output)
{
    Observation observation;
    QEventLoop loop;
    const auto connection = QObject::connect(
        &runner, &omrscope::ScoreExportRunner::exportFinished,
        [&](const omrscope::ScoreExportResult &result) {
            ++observation.terminals;
            observation.result = result;
            loop.quit();
        });
    qputenv("OMRSCOPE_EXPORT_TEST_MODE", mode.toUtf8());
    omrscope::ScoreExportResult preflight;
    observation.started = runner.startJava(input, 1, output, &preflight);
    if (observation.started && observation.terminals == 0) {
        QTimer::singleShot(5000, &loop, &QEventLoop::quit);
        loop.exec();
    }
    qunsetenv("OMRSCOPE_EXPORT_TEST_MODE");
    QObject::disconnect(connection);
    return observation;
}

bool waitForFile(const QString &path, int timeoutMillis)
{
    QElapsedTimer elapsed;
    elapsed.start();
    while (elapsed.elapsed() < timeoutMillis) {
        QCoreApplication::processEvents(QEventLoop::AllEvents, 20);
        if (QFileInfo::exists(path)) {
            return true;
        }
        QThread::msleep(5);
    }
    return QFileInfo::exists(path);
}

} // namespace

int main(int argc, char **argv)
{
    QCoreApplication application(argc, argv);
    const QString producerMode = qEnvironmentVariable("OMRSCOPE_EXPORT_TEST_MODE");
    if (!producerMode.isEmpty()) {
        return fakeGradle(producerMode, application.arguments());
    }

    bool ok = true;
    QString parseError;
    const QString successLine = QStringLiteral(
        "@omrscope-export {\"schema\":1,\"engine\":\"java\","
        "\"event\":\"export_finished\",\"stage\":\"PAGE\",\"success\":true,"
        "\"sheet\":2,\"format\":\"mxl\",\"path\":\"/tmp/result.mxl\","
        "\"bytes\":17,\"sha256\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\","
        "\"elapsed_ms\":3.5}");
    const auto parsed = omrscope::parseScoreExportMarker(successLine, &parseError);
    ok &= check(parsed && parsed->success && parsed->sheet == 2 && parsed->bytes == 17,
                "schema-1 success marker retains the artifact certificate");
    ok &= check(!omrscope::parseScoreExportMarker(
                    successLine + QStringLiteral(" trailing"), &parseError)
                    && !parseError.isEmpty(),
                "trailing non-JSON data cannot be hidden in a marker");
    ok &= check(!omrscope::parseScoreExportMarker(
                    QStringLiteral("@omrscope {\"schema\":1}"), &parseError),
                "recognition-stream prefix is never accepted as export");

    QTemporaryDir repository;
    QTemporaryDir artifacts;
    ok &= check(repository.isValid() && artifacts.isValid(),
                "temporary repository and artifact roots are available");
    QDir root(repository.path());
    const QString gradlew = root.filePath(QStringLiteral("gradlew"));
    ok &= check(installExecutable(gradlew), "fake Gradle launcher is installed");
    const QString input = root.filePath(QStringLiteral("input page.png"));
    {
        QFile file(input);
        ok &= check(file.open(QIODevice::WriteOnly), "synthetic input is writable");
        file.write("not decoded by fake Gradle\n");
    }

    omrscope::ScoreExportRunner runner(root);
    runner.setJavaHome(QStringLiteral("synthetic-java-home"));

    // A successful start owns exactly one terminal. A repeated start is a synchronous busy
    // rejection and must not poison or replace the first attempt's signal.
    const QString repeatedOutput = artifacts.filePath(QStringLiteral("repeated.mxl"));
    Observation repeated;
    QEventLoop repeatedLoop;
    const auto repeatedConnection = QObject::connect(
        &runner, &omrscope::ScoreExportRunner::exportFinished,
        [&](const omrscope::ScoreExportResult &result) {
            ++repeated.terminals;
            repeated.result = result;
            repeatedLoop.quit();
        });
    qputenv("OMRSCOPE_EXPORT_TEST_MODE", QByteArrayLiteral("delay"));
    omrscope::ScoreExportResult firstError;
    omrscope::ScoreExportResult busyError;
    repeated.started = runner.startJava(input, 1, repeatedOutput, &firstError);
    const bool repeatedStarted = runner.startJava(
        input, 1, artifacts.filePath(QStringLiteral("must-not-start.mxl")), &busyError);
    ok &= check(repeated.started && !repeatedStarted
                    && busyError.errorCode == QLatin1String("busy"),
                "repeated start is rejected synchronously as busy");
    QTimer::singleShot(5000, &repeatedLoop, &QEventLoop::quit);
    repeatedLoop.exec();
    qunsetenv("OMRSCOPE_EXPORT_TEST_MODE");
    QObject::disconnect(repeatedConnection);
    ok &= check(repeated.terminals == 1 && repeated.result.success,
                "busy rejection leaves the original attempt's one terminal intact");

    const Observation badHash = run(runner, QStringLiteral("bad-hash"), input,
                                    artifacts.filePath(QStringLiteral("bad-hash.mxl")));
    ok &= check(badHash.started && badHash.terminals == 1 && !badHash.result.success
                    && badHash.result.errorCode == QLatin1String("artifact_mismatch"),
                "local SHA mismatch rejects a nominal success marker");

    const Observation redirected = run(runner, QStringLiteral("redirect"), input,
                                       artifacts.filePath(QStringLiteral("redirect.mxl")));
    ok &= check(redirected.terminals == 1 && !redirected.result.success
                    && redirected.result.errorCode == QLatin1String("artifact_mismatch"),
                "unrequested marker path is never exposed as an artifact");

    const Observation duplicate = run(runner, QStringLiteral("duplicate"), input,
                                      artifacts.filePath(QStringLiteral("duplicate.mxl")));
    ok &= check(duplicate.terminals == 1 && !duplicate.result.success
                    && duplicate.result.errorCode == QLatin1String("protocol_error"),
                "a second terminal marker fails the protocol exactly once");

    const Observation failure = run(runner, QStringLiteral("failure"), input,
                                    artifacts.filePath(QStringLiteral("failure.mxl")));
    ok &= check(failure.terminals == 1 && !failure.result.success
                    && failure.result.errorCode == QLatin1String("page_failed")
                    && failure.result.message.contains(QStringLiteral("synthetic")),
                "probe failure marker plus nonzero exit preserves its stable error");

    const Observation badExit = run(runner, QStringLiteral("bad-exit"), input,
                                    artifacts.filePath(QStringLiteral("bad-exit.mxl")));
    ok &= check(badExit.terminals == 1 && !badExit.result.success
                    && badExit.result.errorCode == QLatin1String("process_failed"),
                "success marker followed by nonzero exit is rejected");

    int uppercaseTerminals = 0;
    const auto uppercaseConnection = QObject::connect(
        &runner, &omrscope::ScoreExportRunner::exportFinished,
        [&uppercaseTerminals](const omrscope::ScoreExportResult &) { ++uppercaseTerminals; });
    omrscope::ScoreExportResult uppercaseError;
    const bool uppercaseStarted = runner.startJava(
        input, 1, artifacts.filePath(QStringLiteral("uppercase.MXL")), &uppercaseError);
    QObject::disconnect(uppercaseConnection);
    ok &= check(!uppercaseStarted && uppercaseTerminals == 0
                    && uppercaseError.errorCode == QLatin1String("invalid_arguments"),
                "uppercase output suffix is synchronously rejected before Java export");

#ifdef Q_OS_UNIX
    // Cancel immediately, without waiting for a child-ready handshake. This exercises the
    // fork-to-setsid window where a process id may exist before its new process group does.
    const QString immediateOutput = artifacts.filePath(QStringLiteral("immediate.mxl"));
    const QString launcherSurvived = immediateOutput + QStringLiteral(".launcher-survived");
    int immediateTerminals = 0;
    omrscope::ScoreExportResult immediateCancellation;
    QEventLoop immediateLoop;
    const auto immediateConnection = QObject::connect(
        &runner, &omrscope::ScoreExportRunner::exportFinished,
        [&](const omrscope::ScoreExportResult &result) {
            ++immediateTerminals;
            immediateCancellation = result;
            immediateLoop.quit();
        });
    qputenv("OMRSCOPE_EXPORT_TEST_MODE", QByteArrayLiteral("immediate-cancel-write"));
    omrscope::ScoreExportResult immediatePreflight;
    const bool immediateStarted = runner.startJava(input, 1, immediateOutput,
                                                    &immediatePreflight);
    if (immediateStarted) {
        runner.cancel();
        QTimer::singleShot(5000, &immediateLoop, &QEventLoop::quit);
        immediateLoop.exec();
    }
    qunsetenv("OMRSCOPE_EXPORT_TEST_MODE");
    QObject::disconnect(immediateConnection);
    QThread::msleep(1000);
    ok &= check(immediateStarted && immediateTerminals == 1
                    && immediateCancellation.errorCode == QLatin1String("cancelled")
                    && !QFileInfo::exists(launcherSurvived),
                "immediate cancellation cannot lose the process-group startup race");

    // A direct QProcess::kill() only terminates gradlew. This fake launcher starts a delayed
    // child that would write after the terminal signal if it escaped. Cancellation must stop the
    // entire new session before the runner reports its one terminal result.
    const QString orphanOutput = artifacts.filePath(QStringLiteral("orphan.mxl"));
    const QString childReady = orphanOutput + QStringLiteral(".child-ready");
    const QString childSurvived = orphanOutput + QStringLiteral(".child-survived");
    int cancellationTerminals = 0;
    omrscope::ScoreExportResult cancellation;
    QEventLoop cancellationLoop;
    const auto cancellationConnection = QObject::connect(
        &runner, &omrscope::ScoreExportRunner::exportFinished,
        [&](const omrscope::ScoreExportResult &result) {
            ++cancellationTerminals;
            cancellation = result;
            cancellationLoop.quit();
        });
    qputenv("OMRSCOPE_EXPORT_TEST_MODE", QByteArrayLiteral("orphan-write"));
    omrscope::ScoreExportResult cancellationPreflight;
    const bool cancellationStarted = runner.startJava(input, 1, orphanOutput,
                                                       &cancellationPreflight);
    const bool childStarted = cancellationStarted && waitForFile(childReady, 1000);
    if (childStarted) {
        runner.cancel();
        QTimer::singleShot(5000, &cancellationLoop, &QEventLoop::quit);
        cancellationLoop.exec();
    }
    qunsetenv("OMRSCOPE_EXPORT_TEST_MODE");
    QObject::disconnect(cancellationConnection);
    // Wait beyond the child's delay, not merely until Gradle exits. A write here proves an
    // orphan survived cancellation and could have recreated a torn-down QTemporaryDir artifact.
    QThread::msleep(1000);
    ok &= check(childStarted && cancellationTerminals == 1 && !cancellation.success
                    && cancellation.errorCode == QLatin1String("cancelled")
                    && !QFileInfo::exists(childSurvived),
                "cancellation kills the Gradle process group before a delayed child can write");
#endif

    // QProcess does not promise `finished` after FailedToStart. The error path emits once and
    // disconnect/delete cleanup must not manufacture a later second terminal.
    QFile::remove(gradlew);
    int startFailureTerminals = 0;
    omrscope::ScoreExportResult startFailure;
    QEventLoop startFailureLoop;
    const auto failedConnection = QObject::connect(
        &runner, &omrscope::ScoreExportRunner::exportFinished,
        [&](const omrscope::ScoreExportResult &result) {
            ++startFailureTerminals;
            startFailure = result;
            QTimer::singleShot(100, &startFailureLoop, &QEventLoop::quit);
        });
    omrscope::ScoreExportResult preflight;
    ok &= check(runner.startJava(input, 1,
                                 artifacts.filePath(QStringLiteral("failed-start.mxl")),
                                 &preflight),
                "missing launcher is an asynchronous failed-to-start attempt");
    QTimer::singleShot(5000, &startFailureLoop, &QEventLoop::quit);
    startFailureLoop.exec();
    QObject::disconnect(failedConnection);
    ok &= check(startFailureTerminals == 1 && !startFailure.success
                    && startFailure.errorCode == QLatin1String("process_start_failed"),
                "failed-to-start cleanup emits exactly one terminal");

    return ok ? 0 : 1;
}
