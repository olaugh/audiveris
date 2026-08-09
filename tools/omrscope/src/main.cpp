// SPDX-License-Identifier: AGPL-3.0-or-later
#include "MainWindow.h"

#include "EngineRunner.h"
#include "Parsers.h"

#include <QApplication>
#include <QDir>
#include <QFileInfo>
#include <QTextStream>

namespace {

/// The repository root, found by walking up until `rust/` and `app/` are both
/// there. Lets the binary be run from anywhere, including a build directory.
QDir findRepository()
{
    QDir directory(QCoreApplication::applicationDirPath());
    for (int depth = 0; depth < 8; ++depth) {
        if (directory.exists(QStringLiteral("rust")) && directory.exists(QStringLiteral("app"))) {
            return directory;
        }
        if (!directory.cdUp()) {
            break;
        }
    }
    return QDir::current();
}

} // namespace

namespace {

/// Runs both engines and prints the comparison, with no window.
///
/// Exists so the tool is scriptable and so a run can be checked from a
/// terminal or a phone. It exercises exactly the same runners and parsers the
/// window uses, which is also what makes it worth having as a smoke test.
int compare(const QDir &repository, const QString &input, int sheet, const QString &step)
{
    QTextStream out(stdout);
    omrscope::EngineRunner runner(repository);

    const omrscope::EngineResult rust = runner.runRust(input, sheet, step);
    const omrscope::EngineResult java = runner.runJava(input, sheet, step);

    auto report = [&out](const omrscope::EngineResult &result) {
        if (!result.error.isEmpty()) {
            out << QStringLiteral("%1: FAILED %2\n").arg(result.engine, result.error);
            return;
        }
        int rejected = 0;
        for (const omrscope::Inter &inter : result.inters) {
            rejected += inter.rejected ? 1 : 0;
        }
        out << QStringLiteral("%1: %2 inters, %3 rejected, %4 relations, %5 staves, "
                              "%6 ms (%7)\n")
                   .arg(result.engine)
                   .arg(result.inters.size() - rejected)
                   .arg(rejected)
                   .arg(result.relationCount)
                   .arg(result.staves.size())
                   .arg(QString::number(result.millis, 'f', 1), result.timingNote);
    };
    report(rust);
    report(java);

    if (!rust.ran || !java.ran) {
        return 1;
    }

    const QVector<omrscope::Pairing> rows = omrscope::pair(rust, java, QString());
    int agree = 0;
    int differ = 0;
    int unpaired = 0;
    int incomparable = 0;
    for (const omrscope::Pairing &row : rows) {
        if (row.rust && row.java) {
            row.agrees() ? ++agree : ++differ;
        } else if (row.rust && row.rust->rejected) {
            continue;
        } else if (row.note == QLatin1String("no geometry to compare")) {
            ++incomparable;
        } else {
            ++unpaired;
        }
    }
    out << QStringLiteral("paired %1, agree %2, differ %3, only-one-side %4, "
                          "not comparable %5\n")
               .arg(agree + differ)
               .arg(agree)
               .arg(differ)
               .arg(unpaired)
               .arg(incomparable);

    for (const omrscope::Pairing &row : rows) {
        if (row.rust && row.java && !row.agrees()) {
            out << QStringLiteral("  differ staff %1 grade %2 vs %3\n")
                       .arg(row.rust->staff)
                       .arg(QString::number(row.rust->grade, 'f', 9),
                            QString::number(row.java->grade, 'f', 9));
        }
    }
    return differ == 0 ? 0 : 2;
}

} // namespace

int main(int argc, char **argv)
{
    QApplication application(argc, argv);
    QCoreApplication::setApplicationName(QStringLiteral("omrscope"));

    QStringList arguments = QCoreApplication::arguments().mid(1);
    QString repositoryPath;
    QString compareInput;
    int sheet = 1;
    QString step = QStringLiteral("GRID");

    for (int at = 0; at < arguments.size(); ++at) {
        const QString &argument = arguments[at];
        if (argument == QLatin1String("--compare") && at + 1 < arguments.size()) {
            compareInput = arguments[++at];
        } else if (argument == QLatin1String("--sheet") && at + 1 < arguments.size()) {
            sheet = arguments[++at].toInt();
        } else if (argument == QLatin1String("--step") && at + 1 < arguments.size()) {
            step = arguments[++at];
        } else if (argument == QLatin1String("--help")) {
            QTextStream(stdout)
                << QStringLiteral(
                       "omrscope [repository] [--compare <sheet-path> [--sheet N] [--step S]]\n"
                       "\n  With --compare, runs both engines and prints the diff, no window.\n");
            return 0;
        } else if (!argument.startsWith(QLatin1Char('-'))) {
            repositoryPath = argument;
        }
    }

    const QDir repository = repositoryPath.isEmpty() ? findRepository() : QDir(repositoryPath);

    if (!compareInput.isEmpty()) {
        return compare(repository, compareInput, sheet, step);
    }

    omrscope::MainWindow window(repository);
    window.show();
    return application.exec();
}
