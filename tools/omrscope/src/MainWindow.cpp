// SPDX-License-Identifier: AGPL-3.0-or-later
#include "MainWindow.h"

#include "PageView.h"
#include "Parsers.h"

#include <QApplication>
#include <QCheckBox>
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
#include <QTreeWidget>
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
    {"HEADERS", "native", "clef, key and time columns compose from live GRID state and publish schema-1 products"},
    {"STEM_SEEDS", "native", "native checker/materialization consumes GRID and HEADERS and publishes accepted seed evidence"},
    {"BEAMS", "native", "beam recognition exact on all 8 sheets: 787/787 raw beams, geometry and six impacts and grade; native STEM_SEEDS feed measured extension, hooks and grouping"},
    {"LEDGERS", "native", "native candidates, filtering and seven-impact grading compose after BEAMS"},
    {"HEADS", "native", "native GRID → HEADERS → STEM_SEEDS → BEAMS → LEDGERS → HEADS composition publishes identity-free final heads"},
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

const QStringList &streamStages()
{
    static const QStringList stages{
        QStringLiteral("GRID"), QStringLiteral("HEADERS"), QStringLiteral("STEM_SEEDS"),
        QStringLiteral("BEAMS"), QStringLiteral("LEDGERS"), QStringLiteral("HEADS"),
    };
    return stages;
}

QString stateText(StageState state)
{
    switch (state) {
    case StageState::NotRequested: return QObject::tr("Not selected");
    case StageState::Queued: return QObject::tr("Queued");
    case StageState::Starting: return QObject::tr("Starting");
    case StageState::Running: return QObject::tr("Running");
    case StageState::Completed: return QObject::tr("Complete");
    case StageState::Failed: return QObject::tr("Failed");
    case StageState::Cancelled: return QObject::tr("Cancelled");
    }
    return {};
}

QString stageCell(const StageSnapshot &snapshot)
{
    QString text = stateText(snapshot.state);
    if (snapshot.state == StageState::Completed) {
        text += QStringLiteral(" · %1 inters · %2 ms")
                    .arg(snapshot.result.inters.size())
                    .arg(QString::number(snapshot.result.millis, 'f', 1));
    } else if ((snapshot.state == StageState::Failed || snapshot.state == StageState::Cancelled)
               && !snapshot.message.isEmpty()) {
        text += QStringLiteral(" · %1").arg(snapshot.message.left(90));
    }
    return text;
}

QString comparisonCell(const StageSnapshot &rust, const StageSnapshot &java)
{
    if (rust.state == StageState::Completed && java.state == StageState::Completed) {
        const QVector<Pairing> rows = pair(rust.result, java.result, QString());
        int different = 0;
        for (const Pairing &row : rows) {
            if (row.rust && row.java && !row.agrees()) {
                ++different;
            }
        }
        return different == 0 ? QObject::tr("Same-stage ready · no grade differences")
                              : QObject::tr("Same-stage ready · %1 grade differences").arg(different);
    }
    return QObject::tr("Waiting for both engines");
}

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

    connect(&runner_, &EngineRunner::stageEvent, this, &MainWindow::stageEvent);
    connect(&runner_, &EngineRunner::streamFinished, this, &MainWindow::streamFinished);
}

void MainWindow::buildUi()
{
    auto *central = new QWidget(this);
    auto *outer = new QVBoxLayout(central);

    // --- controls -------------------------------------------------------
    auto *controls = new QHBoxLayout;
    input_ = new QComboBox;
    input_->setMinimumWidth(360);
    input_->setAccessibleName(tr("Recognition input"));
    sheet_ = new QSpinBox;
    sheet_->setRange(1, 999);
    sheet_->setPrefix(tr("sheet "));
    sheet_->setAccessibleName(tr("Sheet number"));
    step_ = new QComboBox;
    step_->addItems({QStringLiteral("GRID"), QStringLiteral("HEADERS"),
                     QStringLiteral("STEM_SEEDS"), QStringLiteral("BEAMS"),
                     QStringLiteral("LEDGERS"), QStringLiteral("HEADS")});
    step_->setAccessibleName(tr("Last recognition stage to run"));
    withRust_ = new QCheckBox(tr("Rust"));
    withRust_->setChecked(true);
    withRust_->setAccessibleName(tr("Run Rust recognition"));
    withJava_ = new QCheckBox(tr("Java"));
    withJava_->setChecked(true);
    withJava_->setAccessibleName(tr("Run Java recognition"));
    runButton_ = new QPushButton(tr("Run"));
    runButton_->setAccessibleName(tr("Run recognition"));
    cancelButton_ = new QPushButton(tr("Cancel"));
    cancelButton_->setEnabled(false);
    cancelButton_->setAccessibleName(tr("Cancel running recognition"));

    controls->addWidget(new QLabel(tr("Sheet:")));
    controls->addWidget(input_, 1);
    controls->addWidget(sheet_);
    controls->addWidget(new QLabel(tr("Through stage:")));
    controls->addWidget(step_);
    controls->addWidget(withRust_);
    controls->addWidget(withJava_);
    controls->addWidget(runButton_);
    controls->addWidget(cancelButton_);
    outer->addLayout(controls);

    progress_ = new QProgressBar;
    progress_->setRange(0, 0); // Indeterminate: neither engine reports progress.
    progress_->setTextVisible(true);
    progress_->hide();
    outer->addWidget(progress_);

    summary_ = new QLabel(tr("Nothing run yet."));
    summary_->setTextFormat(Qt::RichText);
    summary_->setWordWrap(true);
    summary_->setAccessibleName(tr("Recognition status"));
    outer->addWidget(summary_);

    // A persistent, selectable timeline is the live state of the run. It is
    // separate from the results tabs so a later failure cannot erase useful
    // completed-stage evidence.
    timeline_ = new QTreeWidget;
    timeline_->setColumnCount(4);
    timeline_->setHeaderLabels({tr("Stage"), tr("Rust"), tr("Java"), tr("Comparison")});
    timeline_->setRootIsDecorated(false);
    timeline_->setSelectionMode(QAbstractItemView::SingleSelection);
    timeline_->setAccessibleName(tr("Recognition stage timeline"));
    timeline_->setAccessibleDescription(
        tr("Select a completed or failed stage to inspect its Rust and Java snapshots."));
    timeline_->setMaximumHeight(205);
    outer->addWidget(timeline_);

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
    table_->setAccessibleName(tr("Same-stage recognition comparison"));
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
    connect(cancelButton_, &QPushButton::clicked, this, &MainWindow::cancel);
    connect(timeline_, &QTreeWidget::itemSelectionChanged, this, &MainWindow::stageSelected);
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
        "<p>Nine of the twenty pipeline stages are native and published through HEADS. The rest have their "
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
    cancelButton_->setEnabled(busy);
    if (busy) {
        progress_->setFormat(what);
    }
}

void MainWindow::run()
{
    if (runner_.isRunning(Engine::Rust) || runner_.isRunning(Engine::Java)) {
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
    stageSnapshots_.clear();
    timelineRows_.clear();
    timeline_->clear();
    const int through = streamStages().indexOf(pendingStep_);
    if (through < 0) {
        summary_->setText(tr("Unsupported streaming stage: %1").arg(pendingStep_));
        return;
    }
    followLatest_ = true;
    selectedStage_.clear();
    rustRequested_ = withRust_->isChecked();
    javaRequested_ = withJava_->isChecked();
    rustFinished_ = false;
    javaFinished_ = false;
    rustSucceeded_ = false;
    javaSucceeded_ = false;
    cancellationRequested_ = false;
    rustTerminalMessage_.clear();
    javaTerminalMessage_.clear();
    for (int at = 0; at <= through; ++at) {
        StagePair pair;
        if (!rustRequested_) {
            pair.rust.state = StageState::NotRequested;
        }
        if (!javaRequested_) {
            pair.java.state = StageState::NotRequested;
        }
        stageSnapshots_.insert(streamStages()[at], pair);
    }
    updateTimeline();
    // The sheet is rasterised on the UI thread on purpose: it is Qt decoding an
    // image, it takes milliseconds, and having the page appear immediately is
    // most of what makes the wait for the engines tolerable.
    page_->setImage(renderInput(pendingInput_, pendingSheet_));
    page_->setResults(rust_, java_);
    table_->setRowCount(0);

    if (!rustRequested_ && !javaRequested_) {
        summary_->setText(tr("Choose Rust, Java, or both."));
        return;
    }
    setBusy(true, tr("Starting Rust and Java recognition…"));
    summary_->setText(tr("Rust and Java are running concurrently; completed stages remain selectable."));
    if (rustRequested_) {
        runner_.startRust(pendingInput_, pendingSheet_, pendingStep_);
    }
    if (javaRequested_) {
        runner_.startJava(pendingInput_, pendingSheet_, pendingStep_);
    }
}

void MainWindow::cancel()
{
    cancellationRequested_ = true;
    runner_.cancel(Engine::Rust);
    runner_.cancel(Engine::Java);
    summary_->setText(tr("Cancelling active stages. Completed snapshots remain available."));
}

void MainWindow::setStageState(Engine engine, const StreamEvent &event,
                               const EngineResult &result)
{
    if (event.stage.isEmpty() || !stageSnapshots_.contains(event.stage)) {
        return;
    }
    StagePair &pair = stageSnapshots_[event.stage];
    StageSnapshot &snapshot = engine == Engine::Rust ? pair.rust : pair.java;
    if (snapshot.state == StageState::Completed) {
        // A completed snapshot is immutable even if a corrupt producer sends
        // a later conflicting marker for the same stage.
        return;
    }
    pair.comparisonReady = false;
    snapshot.result = result;
    snapshot.message = event.message;
    snapshot.sequence = event.sequence;
    if (event.event == QLatin1String("stage_started")) {
        snapshot.state = StageState::Running;
    } else if (event.event == QLatin1String("stage_completed")) {
        snapshot.state = StageState::Completed;
    } else if (event.event == QLatin1String("stage_cancelled")) {
        snapshot.state = StageState::Cancelled;
    } else if (event.event == QLatin1String("stage_failed")) {
        snapshot.state = StageState::Failed;
    }
}

void MainWindow::stageEvent(Engine engine, StreamEvent event, EngineResult result)
{
    setStageState(engine, event, result);
    updateTimeline();
    if (followLatest_) {
        followNewestCommonStage();
        if (selectedStage_.isEmpty() && !event.stage.isEmpty()) {
            selectStage(event.stage);
        }
    }

    const QString engineName = engine == Engine::Rust ? tr("Rust") : tr("Java");
    if (event.event == QLatin1String("stage_started")) {
        summary_->setText(tr("%1 is running %2.").arg(engineName, event.stage));
    } else if (event.event == QLatin1String("stage_failed")) {
        summary_->setText(tr("%1 failed at %2: %3").arg(engineName, event.stage, event.message));
    }
}

void MainWindow::streamFinished(Engine engine, bool success, bool cancelled,
                                const QString &message)
{
    if (engine == Engine::Rust) {
        rustFinished_ = true;
        rustSucceeded_ = success;
        rustTerminalMessage_ = message;
    } else {
        javaFinished_ = true;
        javaSucceeded_ = success;
        javaTerminalMessage_ = message;
    }
    // A stopped stream can leave later rows queued. They were not executed;
    // make that explicit while preserving all completed predecessors.
    for (const QString &stage : streamStages()) {
        if (!stageSnapshots_.contains(stage)) {
            continue;
        }
        StageSnapshot &snapshot = engine == Engine::Rust
            ? stageSnapshots_[stage].rust : stageSnapshots_[stage].java;
        if (snapshot.state == StageState::Queued || snapshot.state == StageState::Starting
            || snapshot.state == StageState::Running) {
            snapshot.state = cancelled ? StageState::Cancelled : StageState::Failed;
            snapshot.message = cancelled ? tr("not reached after cancellation")
                                         : message;
        }
    }
    updateTimeline();
    if (followLatest_) {
        followNewestCommonStage();
    }
    const bool busy = (rustRequested_ && !rustFinished_) || (javaRequested_ && !javaFinished_);
    setBusy(busy, tr("Recognition running…"));
    if (!busy) {
        if (!selectedStage_.isEmpty()) {
            selectStage(selectedStage_);
        } else {
            summary_->setText(terminalSummaryHtml());
        }
    } else if (!success) {
        const QString engineLabel = engine == Engine::Rust ? tr("Rust") : tr("Java");
        summary_->setText(tr("%1 run failed while the other engine continues: %2")
                              .arg(engineLabel, message.toHtmlEscaped()));
    }
}

QString MainWindow::terminalSummaryHtml() const
{
    const bool finished = (!rustRequested_ || rustFinished_)
        && (!javaRequested_ || javaFinished_);
    if (!finished) {
        return {};
    }
    if (cancellationRequested_) {
        return tr("<b>Run</b>: cancelled; completed stages are retained.");
    }
    QStringList failures;
    if (rustRequested_ && !rustSucceeded_) {
        failures << tr("Rust: %1").arg(rustTerminalMessage_.toHtmlEscaped());
    }
    if (javaRequested_ && !javaSucceeded_) {
        failures << tr("Java: %1").arg(javaTerminalMessage_.toHtmlEscaped());
    }
    if (!failures.isEmpty()) {
        return tr("<b>Run failed</b>: %1").arg(failures.join(QStringLiteral("; ")));
    }
    return tr("<b>Run</b>: complete.");
}

void MainWindow::updateTimeline()
{
    for (const QString &stage : streamStages()) {
        if (!stageSnapshots_.contains(stage)) {
            continue;
        }
        QTreeWidgetItem *row = timelineRows_.value(stage, nullptr);
        if (!row) {
            row = new QTreeWidgetItem(timeline_);
            row->setData(0, Qt::UserRole, stage);
            timelineRows_.insert(stage, row);
        }
        StagePair &pair = stageSnapshots_[stage];
        if (!pair.comparisonReady && pair.rust.state == StageState::Completed
            && pair.java.state == StageState::Completed) {
            pair.comparison = comparisonCell(pair.rust, pair.java);
            pair.comparisonReady = true;
        }
        row->setText(0, stage);
        row->setText(1, stageCell(pair.rust));
        row->setText(2, stageCell(pair.java));
        row->setText(3, pair.comparisonReady ? pair.comparison
                                             : comparisonCell(pair.rust, pair.java));
        row->setToolTip(1, pair.rust.message);
        row->setToolTip(2, pair.java.message);
    }
    timeline_->resizeColumnToContents(0);
    timeline_->resizeColumnToContents(1);
    timeline_->resizeColumnToContents(2);
}

void MainWindow::followNewestCommonStage()
{
    QString newest;
    for (const QString &stage : streamStages()) {
        if (!stageSnapshots_.contains(stage)) {
            continue;
        }
        const StagePair &pair = stageSnapshots_[stage];
        const bool ready = rustRequested_ && javaRequested_
            ? pair.rust.state == StageState::Completed && pair.java.state == StageState::Completed
            : rustRequested_ ? pair.rust.state == StageState::Completed
                             : pair.java.state == StageState::Completed;
        if (ready) {
            newest = stage;
        }
    }
    if (!newest.isEmpty()) {
        selectStage(newest);
    }
}

void MainWindow::stageSelected()
{
    const QList<QTreeWidgetItem *> selected = timeline_->selectedItems();
    if (selected.isEmpty()) {
        return;
    }
    selectStage(selected.first()->data(0, Qt::UserRole).toString(), !selectingProgrammatically_);
}

void MainWindow::selectStage(const QString &stage, bool fromUser)
{
    if (!stageSnapshots_.contains(stage)) {
        return;
    }
    if (fromUser) {
        followLatest_ = false;
    }
    selectedStage_ = stage;
    QTreeWidgetItem *row = timelineRows_.value(stage, nullptr);
    if (row && timeline_->currentItem() != row) {
        selectingProgrammatically_ = true;
        timeline_->setCurrentItem(row);
        selectingProgrammatically_ = false;
    }
    const StagePair &pair = stageSnapshots_[stage];
    rust_ = pair.rust.result;
    java_ = pair.java.result;
    rust_.engine = QStringLiteral("rust");
    java_.engine = QStringLiteral("java");
    if (pair.rust.state != StageState::Completed && rust_.error.isEmpty()) {
        rust_.error = stateText(pair.rust.state);
    }
    if (pair.java.state != StageState::Completed && java_.error.isEmpty()) {
        java_.error = stateText(pair.java.state);
    }
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

    QString summary = QStringLiteral("<b>%1</b><br>").arg(selectedStage_.toHtmlEscaped())
        + describeEngine(rust_, QStringLiteral("Rust"))
        + QStringLiteral("<br>") + describeEngine(java_, QStringLiteral("Java"));

    if (rust_.ran && java_.ran && rust_.millis > 0.0 && java_.millis > 0.0) {
        summary += QStringLiteral("<br><b>Speed</b>: Rust is %1&times; Java's recognition time"
                                  " (like for like on the engine, not the process).")
                       .arg(QString::number(java_.millis / rust_.millis, 'f', 1));
    }
    const QString terminal = terminalSummaryHtml();
    if (!terminal.isEmpty()) {
        summary += QStringLiteral("<br>") + terminal;
    }
    summary_->setText(summary);

    log_->setPlainText(QStringLiteral("=== Rust %1 ===\n%2\n\n=== Java %1 ===\n%3")
                           .arg(selectedStage_, rust_.raw.isEmpty() ? rust_.error : rust_.raw,
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
