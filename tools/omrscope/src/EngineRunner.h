// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include "Model.h"

#include <QDir>
#include <QObject>
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
    EngineResult runRust(const QString &input, int sheet);

    /// Java through Gradle, timed *inside* the probe.
    ///
    /// Wall clock here is dominated by Gradle and JVM startup -- tens of
    /// seconds against a few hundred milliseconds of actual work -- so the
    /// probe reports its own `reachStep` duration and that is what is shown.
    /// Comparing the two wall clocks would say nothing about either engine.
    EngineResult runJava(const QString &input, int sheet, const QString &step);

    void setJavaHome(const QString &path) { javaHome_ = path; }
    QString javaHome() const { return javaHome_; }

    /// Where the Rust release binary should be, whether or not it exists.
    QString rustBinary() const;

    static QString findJavaHome();

private:
    QDir repository_;
    QString javaHome_;
};

} // namespace omrscope
