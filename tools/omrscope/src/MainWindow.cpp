// SPDX-License-Identifier: AGPL-3.0-or-later
#include "MainWindow.h"

#include "PageView.h"
#include "Parsers.h"

#include <QApplication>
#include <QCheckBox>
#include <QtConcurrent/QtConcurrentRun>
#include <QComboBox>
#include <QFile>
#include <QFileInfo>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QLabel>
#include <QPdfDocument>
#include <QPdfDocumentRenderOptions>
#include <QPlainTextEdit>
#include <QProgressBar>
#include <QPushButton>
#include <QSpinBox>
#include <QSplitter>
#include <QTabWidget>
#include <QTableWidget>
#include <QTextBrowser>
#include <QVBoxLayout>

namespace omrscope {

namespace {

/// The pipeline, and which stages the port actually implements.
///
/// Kept here rather than parsed out of PORTING.md: the table is prose, and a
/// dashboard that silently mis-parses prose is worse than one that is
/// explicitly a snapshot someone has to update.
struct StageStatus
{
    const char *name;
    const char *state;   ///< "native", "lifecycle", or "queued"
    const char *note;
};

constexpr StageStatus kStages[] = {
    {"LOAD", "native", "PNG, JPEG and PDF; PDF renders bit-exactly against PDFBox on 189 pages"},
    {"BINARY", "native", "adaptive threshold; 9/9 example rasters bit-identical"},
    {"SCALE", "native", "line, interline and beam estimates exact on 4 pages and the branch cases"},
    {"GRID", "native", "staves, lines, barlines, systems, SIG; 420/420 barlines, all grades exact"},
    {"HEADERS", "lifecycle", "candidate sourcing, classifier ranking and glyph components all native; needs them wired to the clef/key/time recognizers"},
    {"STEM_SEEDS", "lifecycle", "stem scale and checker native; raw StickFactory geometry is a seam"},
    {"BEAMS", "native", "beam recognition exact on all 8 sheets: 787/787 raw beams, geometry and six impacts and grade. The only end-of-step gap is multiple-rest detection, a separate recogniser that consumes one beam. extendToStem awaits STEM_SEEDS"},
    {"LEDGERS", "lifecycle", "filter, candidates and all seven impacts ported; blocked on BEAMS"},
    {"HEADS", "lifecycle", "blocked on MusicFont: head recognition template-matches font-derived symbols, and Java itself cannot reach HEADS without them"},
    {"STEMS", "lifecycle", ""},
    {"REDUCTION", "lifecycle", ""},
    {"CUE_BEAMS", "lifecycle", ""},
    {"TEXTS", "lifecycle", "needs Tesseract"},
    {"MEASURES", "lifecycle", ""},
    {"CHORDS", "lifecycle", ""},
    {"CURVES", "lifecycle", ""},
    {"SYMBOLS", "lifecycle", ""},
    {"LINKS", "lifecycle", ""},
    {"RHYTHMS", "lifecycle", ""},
    {"PAGE", "lifecycle", "no MusicXML export at all"},
};

QString colourFor(const QString &state)
{
    if (state == QLatin1String("native")) {
        return QStringLiteral("#26a65b");
    }
    if (state == QLatin1String("lifecycle")) {
        return QStringLiteral("#f39c12");
    }
    return QStringLiteral("#c0392b");
}

} // namespace

MainWindow::MainWindow(QDir repository, QWidget *parent)
    : QMainWindow(parent)
    , repository_(repository)
    , runner_(repository, this)
{
    setWindowTitle(tr("omrscope — Audiveris port scope"));
    resize(1500, 950);
    buildUi();
    loadInputs();
    loadStatus();

    connect(&rustWatcher_, &QFutureWatcher<EngineResult>::finished, this,
            &MainWindow::rustFinished);
    connect(&javaWatcher_, &QFutureWatcher<EngineResult>::finished, this,
            &MainWindow::javaFinished);
}

void MainWindow::buildUi()
{
    auto *central = new QWidget(this);
    auto *outer = new QVBoxLayout(central);

    // --- controls -------------------------------------------------------
    auto *controls = new QHBoxLayout;
    input_ = new QComboBox;
    input_->setMinimumWidth(360);
    sheet_ = new QSpinBox;
    sheet_->setRange(1, 999);
    sheet_->setPrefix(tr("sheet "));
    step_ = new QComboBox;
    step_->addItems({QStringLiteral("GRID"), QStringLiteral("HEADERS"),
                     QStringLiteral("STEM_SEEDS"), QStringLiteral("BEAMS"),
                     QStringLiteral("LEDGERS")});
    withRust_ = new QCheckBox(tr("Rust"));
    withRust_->setChecked(true);
    withJava_ = new QCheckBox(tr("Java"));
    withJava_->setChecked(true);
    runButton_ = new QPushButton(tr("Run"));

    controls->addWidget(new QLabel(tr("Sheet:")));
    controls->addWidget(input_, 1);
    controls->addWidget(sheet_);
    controls->addWidget(new QLabel(tr("Java step:")));
    controls->addWidget(step_);
    controls->addWidget(withRust_);
    controls->addWidget(withJava_);
    controls->addWidget(runButton_);
    outer->addLayout(controls);

    progress_ = new QProgressBar;
    progress_->setRange(0, 0); // Indeterminate: neither engine reports progress.
    progress_->setTextVisible(true);
    progress_->hide();
    outer->addWidget(progress_);

    summary_ = new QLabel(tr("Nothing run yet."));
    summary_->setTextFormat(Qt::RichText);
    summary_->setWordWrap(true);
    outer->addWidget(summary_);

    // --- tabs -----------------------------------------------------------
    auto *tabs = new QTabWidget;

    // Page
    auto *pageTab = new QWidget;
    auto *pageLayout = new QVBoxLayout(pageTab);
    auto *toggles = new QHBoxLayout;
    auto *showRust = new QCheckBox(tr("Rust marks"));
    showRust->setChecked(true);
    auto *showJava = new QCheckBox(tr("Java marks"));
    showJava->setChecked(true);
    auto *showRejected = new QCheckBox(tr("Rejected candidates"));
    showRejected->setChecked(true);
    auto *showStaves = new QCheckBox(tr("Staff lines"));
    showStaves->setChecked(true);
    toggles->addWidget(showRust);
    toggles->addWidget(showJava);
    toggles->addWidget(showRejected);
    toggles->addWidget(showStaves);
    toggles->addStretch(1);
    toggles->addWidget(new QLabel(
        tr("<span style='color:#26a65b'>■</span> agree  "
           "<span style='color:#2980b9'>■</span> Rust only  "
           "<span style='color:#c0392b'>■</span> Java only  "
           "<span style='color:#f39c12'>▨</span> rejected candidate")));
    pageLayout->addLayout(toggles);
    page_ = new PageView;
    pageLayout->addWidget(page_, 1);
    tabs->addTab(pageTab, tr("Page"));

    connect(showRust, &QCheckBox::toggled, page_, &PageView::setShowRust);
    connect(showJava, &QCheckBox::toggled, page_, &PageView::setShowJava);
    connect(showRejected, &QCheckBox::toggled, page_, &PageView::setShowRejected);
    connect(showStaves, &QCheckBox::toggled, page_, &PageView::setShowStaves);

    // Inters
    auto *interTab = new QWidget;
    auto *interLayout = new QVBoxLayout(interTab);
    auto *filterRow = new QHBoxLayout;
    filter_ = new QComboBox;
    filter_->setEditable(true);
    filter_->addItems({QString(), QStringLiteral("BARLINE"), QStringLiteral("BEAM"),
                       QStringLiteral("LEDGER"), QStringLiteral("HEAD"),
                       QStringLiteral("CONNECTOR")});
    filterRow->addWidget(new QLabel(tr("Kind contains:")));
    filterRow->addWidget(filter_, 1);
    interLayout->addLayout(filterRow);
    table_ = new QTableWidget;
    table_->setAlternatingRowColors(true);
    table_->setEditTriggers(QAbstractItemView::NoEditTriggers);
    interLayout->addWidget(table_, 1);
    tabs->addTab(interTab, tr("Inters"));
    connect(filter_, &QComboBox::currentTextChanged, this, &MainWindow::refreshFilter);

    // Status
    status_ = new QTextBrowser;
    status_->setOpenExternalLinks(true);
    tabs->addTab(status_, tr("Port status"));

    // Log
    log_ = new QPlainTextEdit;
    log_->setReadOnly(true);
    log_->setLineWrapMode(QPlainTextEdit::NoWrap);
    tabs->addTab(log_, tr("Raw output"));

    outer->addWidget(tabs, 1);
    setCentralWidget(central);

    connect(runButton_, &QPushButton::clicked, this, &MainWindow::run);
}

void MainWindow::loadInputs()
{
    // The example pages the oracles already cover, plus any PDF corpus the
    // environment points at.
    const QDir examples(repository_.filePath(QStringLiteral("data/examples")));
    for (const QFileInfo &info :
         examples.entryInfoList({QStringLiteral("*.png"), QStringLiteral("*.jpg"),
                                 QStringLiteral("*.pdf")},
                                QDir::Files, QDir::Name)) {
        input_->addItem(QStringLiteral("data/examples/%1").arg(info.fileName()));
    }

    const QString corpus = qEnvironmentVariable("AUDIVERIS_PDF_CORPUS");
    if (!corpus.isEmpty()) {
        const QDir directory(corpus);
        for (const QFileInfo &info :
             directory.entryInfoList({QStringLiteral("*.pdf")}, QDir::Files, QDir::Name)) {
            input_->addItem(info.absoluteFilePath());
        }
    }
}

void MainWindow::loadStatus()
{
    QString html = QStringLiteral(
        "<h2>Port status</h2>"
        "<p>Four of the twenty pipeline stages are native. The rest have their "
        "step lifecycle, ownership and failure semantics ported, but not the "
        "recognition inside them &mdash; so they run, and produce nothing.</p>"
        "<table cellpadding='6' cellspacing='0' width='100%'>"
        "<tr><th align='left'>Stage</th><th align='left'>State</th>"
        "<th align='left'>Notes</th></tr>");
    for (const StageStatus &stage : kStages) {
        html += QStringLiteral("<tr><td><b>%1</b></td>"
                               "<td><span style='color:%2'>&#9632;</span> %3</td>"
                               "<td>%4</td></tr>")
                    .arg(QString::fromLatin1(stage.name), colourFor(QString::fromLatin1(stage.state)),
                         QString::fromLatin1(stage.state), QString::fromLatin1(stage.note));
    }
    html += QStringLiteral("</table>");

    html += QStringLiteral(
        "<h3>What &ldquo;exact&rdquo; means</h3>"
        "<p>Not a tolerance. Hashes, or value-for-value comparison against a "
        "live Java run. The oracles under <code>rust/oracle/</code> hold Java's "
        "answers; the probes that generate them sit beside the data.</p>"
        "<ul>"
        "<li><b>PDF</b> &mdash; 189/189 corpus pages at four depths: raw bytes, "
        "filtered bytes, decoded samples, rendered page.</li>"
        "<li><b>JPEG</b> &mdash; sample for sample against libjpeg 6b, the one "
        "Audiveris bundles.</li>"
        "<li><b>GRID</b> &mdash; 420/420 barline abscissae, 1300/1300 line "
        "endpoints, every grade, and the staff-free image on all nine pages.</li>"
        "<li><b>Staff areas</b> &mdash; 1209/1209 lattice points agree with "
        "<code>getClosestStaff</code>.</li>"
        "</ul>"
        "<h3>Timing</h3>"
        "<p>The two numbers are not the same measurement, and the tool says so "
        "rather than putting them in one column. Rust is process wall clock, "
        "whose startup is a few milliseconds. Java is measured <i>inside</i> the "
        "probe, around <code>reachStep</code> only, because Gradle and JVM "
        "startup take tens of seconds and would swamp the comparison.</p>");
    status_->setHtml(html);
}

QImage MainWindow::renderInput(const QString &input, int sheet) const
{
    const QString path = QFileInfo(input).isAbsolute() ? input : repository_.filePath(input);
    if (!path.endsWith(QLatin1String(".pdf"), Qt::CaseInsensitive)) {
        return QImage(path);
    }

    // A PDF sheet is rendered here only so there is something to draw on.
    // It is Qt's rasterizer, not the port's, so it is *not* evidence about
    // ingest -- the corpus test is what grades that.
    QPdfDocument document;
    if (document.load(path) != QPdfDocument::Error::None) {
        return {};
    }
    const int index = sheet - 1;
    if (index < 0 || index >= document.pageCount()) {
        return {};
    }
    const QSizeF points = document.pagePointSize(index);
    const double scale = 300.0 / 72.0;
    return document.render(index, QSize(qRound(points.width() * scale),
                                        qRound(points.height() * scale)));
}

void MainWindow::setBusy(bool busy, const QString &what)
{
    runButton_->setEnabled(!busy);
    input_->setEnabled(!busy);
    sheet_->setEnabled(!busy);
    step_->setEnabled(!busy);
    withRust_->setEnabled(!busy);
    withJava_->setEnabled(!busy);
    progress_->setVisible(busy);
    if (busy) {
        progress_->setFormat(what);
    }
}

void MainWindow::run()
{
    if (rustWatcher_.isRunning() || javaWatcher_.isRunning()) {
        return;
    }
    pendingInput_ = input_->currentText();
    pendingSheet_ = sheet_->value();
    pendingStep_ = step_->currentText();
    if (pendingInput_.isEmpty()) {
        return;
    }

    rust_ = EngineResult{};
    java_ = EngineResult{};
    // The sheet is rasterised on the UI thread on purpose: it is Qt decoding an
    // image, it takes milliseconds, and having the page appear immediately is
    // most of what makes the wait for the engines tolerable.
    page_->setImage(renderInput(pendingInput_, pendingSheet_));
    page_->setResults(rust_, java_);
    table_->setRowCount(0);

    if (withRust_->isChecked()) {
        setBusy(true, tr("Running Rust…"));
        summary_->setText(tr("Running Rust…"));
        rustWatcher_.setFuture(QtConcurrent::run([this] {
            // Only reads immutable state on the runner, so it is safe off the
            // UI thread; the QProcess it creates lives entirely in this task.
            return runner_.runRust(pendingInput_, pendingSheet_);
        }));
        return;
    }
    startJava();
}

void MainWindow::rustFinished()
{
    rust_ = rustWatcher_.result();
    // Show the Rust half at once rather than making it wait on Gradle, which
    // can take a minute and has nothing to do with it.
    showResults();
    startJava();
}

void MainWindow::startJava()
{
    if (!withJava_->isChecked()) {
        setBusy(false);
        showResults();
        return;
    }
    setBusy(true, tr("Running Java (Gradle startup dominates this)…"));
    javaWatcher_.setFuture(QtConcurrent::run([this] {
        return runner_.runJava(pendingInput_, pendingSheet_, pendingStep_);
    }));
}

void MainWindow::javaFinished()
{
    java_ = javaWatcher_.result();
    setBusy(false);
    showResults();
}

void MainWindow::showResults()
{
    page_->setResults(rust_, java_);

    auto describeEngine = [](const EngineResult &result, const QString &label) {
        if (!result.ran && result.error.isEmpty()) {
            return QStringLiteral("<b>%1</b>: not run").arg(label);
        }
        if (!result.error.isEmpty()) {
            return QStringLiteral("<b>%1</b>: <span style='color:#c0392b'>%2</span>")
                .arg(label, result.error.toHtmlEscaped().left(300));
        }
        int rejected = 0;
        for (const Inter &inter : result.inters) {
            rejected += inter.rejected ? 1 : 0;
        }
        // Java purges the same candidates -- the port's rejection reasons are
        // Java's own stage names -- but by the time a SIG exists the purged
        // peaks are gone, so there is nothing to report. Printing "0 rejected"
        // would read as "rejected nothing", which is a different claim.
        const QString rejectedText = result.engine == QLatin1String("java")
            ? QStringLiteral("rejections not reported")
            : QStringLiteral("%1 rejected").arg(rejected);
        return QStringLiteral("<b>%1</b>: %2 inters (%3), %4 relations, "
                              "%5 staves &mdash; <b>%6 ms</b> <i>(%7)</i>")
            .arg(label)
            .arg(result.inters.size() - rejected)
            .arg(rejectedText)
            .arg(result.relationCount)
            .arg(result.staves.size())
            .arg(QString::number(result.millis, 'f', 1), result.timingNote);
    };

    QString summary = describeEngine(rust_, QStringLiteral("Rust"))
        + QStringLiteral("<br>") + describeEngine(java_, QStringLiteral("Java"));

    if (rust_.ran && java_.ran && rust_.millis > 0.0 && java_.millis > 0.0) {
        summary += QStringLiteral("<br><b>Speed</b>: Rust is %1&times; Java's recognition time"
                                  " (like for like on the engine, not the process).")
                       .arg(QString::number(java_.millis / rust_.millis, 'f', 1));
    }
    summary_->setText(summary);

    log_->setPlainText(QStringLiteral("=== Rust ===\n%1\n\n=== Java ===\n%2")
                           .arg(rust_.raw.isEmpty() ? rust_.error : rust_.raw,
                                java_.raw.isEmpty() ? java_.error : java_.raw));
    refreshFilter();
}

void MainWindow::refreshFilter()
{
    const QVector<Pairing> rows = pair(rust_, java_, filter_->currentText().trimmed());

    const QStringList headers{tr("Kind"),        tr("Staff"),      tr("Rust x"),
                              tr("Java x"),      tr("Rust grade"), tr("Java grade"),
                              tr("Δ grade"),     tr("Rust impacts"), tr("Java impacts"),
                              tr("Note")};
    table_->clear();
    table_->setColumnCount(headers.size());
    table_->setHorizontalHeaderLabels(headers);
    table_->setRowCount(rows.size());

    auto impacts = [](const std::optional<Inter> &inter) {
        if (!inter) {
            return QString();
        }
        QStringList parts;
        for (const Impact &impact : inter->impacts) {
            parts << QStringLiteral("%1 %2").arg(impact.name, QString::number(impact.value, 'f', 4));
        }
        return parts.join(QStringLiteral("  "));
    };
    auto abscissa = [](const std::optional<Inter> &inter) {
        if (!inter) {
            return QString();
        }
        if (inter->median) {
            return QString::number(inter->median->center().x(), 'f', 1);
        }
        if (inter->bounds) {
            return QString::number(inter->bounds->center().x(), 'f', 1);
        }
        return QString();
    };

    for (int row = 0; row < rows.size(); ++row) {
        const Pairing &pairing = rows[row];
        const std::optional<Inter> &any = pairing.rust ? pairing.rust : pairing.java;
        const QString delta = (pairing.rust && pairing.java)
            ? QString::number(pairing.rust->grade - pairing.java->grade, 'e', 2)
            : QString();

        const QStringList cells{
            any ? any->kind : QString(),
            any && any->staff >= 0 ? QString::number(any->staff) : QString(),
            abscissa(pairing.rust),
            abscissa(pairing.java),
            pairing.rust ? QString::number(pairing.rust->grade, 'f', 9) : QString(),
            pairing.java ? QString::number(pairing.java->grade, 'f', 9) : QString(),
            delta,
            impacts(pairing.rust),
            impacts(pairing.java),
            pairing.note,
        };
        for (int column = 0; column < cells.size(); ++column) {
            auto *item = new QTableWidgetItem(cells[column]);
            // Only disagreement is worth colour. Agreement is the default and
            // should be quiet, or the eye has nothing to catch on.
            if (pairing.rust && pairing.java && !pairing.agrees()) {
                item->setBackground(QColor(192, 57, 43, 60));
            } else if (!pairing.note.isEmpty()) {
                item->setBackground(QColor(243, 156, 18, 50));
            }
            table_->setItem(row, column, item);
        }
    }
    table_->resizeColumnsToContents();
}

} // namespace omrscope
