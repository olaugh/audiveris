// SPDX-License-Identifier: AGPL-3.0-or-later
#include "EngineRunner.h"

#include "Parsers.h"

#include <QElapsedTimer>
#include <QFileInfo>
#include <QProcess>
#include <QProcessEnvironment>

namespace omrscope {

namespace {

/// Java probes take a long time; a short timeout would look like a failure.
constexpr int kJavaTimeoutMs = 15 * 60 * 1000;
constexpr int kRustTimeoutMs = 5 * 60 * 1000;

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

EngineResult EngineRunner::runRust(const QString &input, int sheet)
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
                                QStringLiteral("GRID"),
                                QStringLiteral("-json"),
                                input,
                                QStringLiteral("-sheets"),
                                QString::number(sheet)};

    QElapsedTimer timer;
    timer.start();
    process.start(binary, arguments);
    if (!process.waitForFinished(kRustTimeoutMs)) {
        result.error = QStringLiteral("timed out");
        return result;
    }
    result.millis = static_cast<double>(timer.nsecsElapsed()) / 1e6;

    const QString output = QString::fromUtf8(process.readAllStandardOutput());
    if (output.trimmed().isEmpty()) {
        result.error = QString::fromUtf8(process.readAllStandardError()).trimmed();
        if (result.error.isEmpty()) {
            result.error = QStringLiteral("no output");
        }
        return result;
    }

    EngineResult parsed = parseRustJson(output);
    parsed.millis = result.millis;
    parsed.timingNote = result.timingNote;
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
    if (!process.waitForFinished(kJavaTimeoutMs)) {
        result.error = QStringLiteral("timed out");
        return result;
    }

    const QString output = QString::fromUtf8(process.readAllStandardOutput());
    EngineResult parsed = parseSigProbe(output);
    parsed.timingNote = result.timingNote;
    if (!parsed.ran && parsed.error.isEmpty()) {
        parsed.error = QString::fromUtf8(process.readAllStandardError()).trimmed();
        if (parsed.error.isEmpty()) {
            parsed.error = QStringLiteral("no records; is the Gradle build working?");
        }
    }
    return parsed;
}

} // namespace omrscope
