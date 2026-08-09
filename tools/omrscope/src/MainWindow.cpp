// SPDX-License-Identifier: AGPL-3.0-or-later
#include "MainWindow.h"

#include "PageView.h"
#include "Parsers.h"
#include "ScoreView.h"

#include <QAccessible>
#include <QAccessibleWidget>
#include <QApplication>
#include <QCheckBox>
#include <QCloseEvent>
#include <QComboBox>
#include <QEvent>
#include <QFile>
#include <QFileDialog>
#include <QFileInfo>
#include <QGroupBox>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QLabel>
#include <QMouseEvent>
#include <QPainter>
#include <QPdfDocument>
#include <QPdfDocumentRenderOptions>
#include <QPlainTextEdit>
#include <QProgressBar>
#include <QPushButton>
#include <QSaveFile>
#include <QSignalBlocker>
#include <QSlider>
#include <QSpinBox>
#include <QSplitter>
#include <QStyledItemDelegate>
#include <QTabWidget>
#include <QTableWidget>
#include <QTextBrowser>
#include <QTreeWidget>
#include <QTimer>
#include <QVBoxLayout>

namespace omrscope {

namespace {

constexpr int kInspectedRole = Qt::UserRole + 1;
constexpr char kFlatItemViewAccessibilityProperty[] = "_omrscope_flat_item_view_accessibility";

class FlatItemViewAccessible final : public QAccessibleWidget
{
public:
    explicit FlatItemViewAccessible(QWidget *widget)
        : QAccessibleWidget(widget, QAccessible::Client)
    {
    }

    int childCount() const override { return 0; }
    QAccessibleInterface *child(int) const override { return nullptr; }
    QAccessibleInterface *childAt(int, int) const override { return nullptr; }
    int indexOfChild(const QAccessibleInterface *) const override { return -1; }
    QAccessibleInterface *focusChild() const override { return nullptr; }

    void *interface_cast(QAccessible::InterfaceType type) override
    {
        if (type == QAccessible::SelectionInterface || type == QAccessible::TableInterface
            || type == QAccessible::TableCellInterface) {
            return nullptr;
        }
        return QAccessibleWidget::interface_cast(type);
    }
};

QAccessibleInterface *flatItemViewFactory(const QString &, QObject *object)
{
    auto *widget = qobject_cast<QWidget *>(object);
    if (!widget || !object->property(kFlatItemViewAccessibilityProperty).toBool()) {
        return nullptr;
    }
    return new FlatItemViewAccessible(widget);
}

void installFlatItemViewFactory()
{
    static const bool installed = [] {
        QAccessible::installFactory(flatItemViewFactory);
        return true;
    }();
    Q_UNUSED(installed);
}

class InspectedRowDelegate final : public QStyledItemDelegate
{
public:
    using QStyledItemDelegate::QStyledItemDelegate;

    void paint(QPainter *painter, const QStyleOptionViewItem &option,
               const QModelIndex &index) const override
    {
        QStyleOptionViewItem base(option);
        base.state &= ~QStyle::State_Selected;
        QStyledItemDelegate::paint(painter, base, index);
        if (!index.data(kInspectedRole).toBool()) {
            return;
        }

        painter->save();
        painter->fillRect(option.rect.adjusted(1, 1, -1, -1), QColor(23, 162, 184, 48));
        painter->setPen(QPen(QColor(23, 162, 184), 2.0));
        const QRectF outline = QRectF(option.rect).adjusted(1.0, 1.0, -1.0, -1.0);
        painter->drawLine(outline.topLeft(), outline.topRight());
        painter->drawLine(outline.bottomLeft(), outline.bottomRight());
        if (index.column() == 0) {
            painter->drawLine(outline.topLeft(), outline.bottomLeft());
        }
        if (index.column() + 1 == index.model()->columnCount()) {
            painter->drawLine(outline.topRight(), outline.bottomRight());
        }
        painter->restore();
    }
};

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
    , scoreExport_(repository, this)
    , scoreRenderer_(this)
{
    setWindowTitle(tr("omrscope — Audiveris port scope"));
    resize(1500, 950);
    buildUi();
    loadInputs();
    loadStatus();

    connect(&runner_, &EngineRunner::stageEvent, this, &MainWindow::stageEvent);
    connect(&runner_, &EngineRunner::streamFinished, this, &MainWindow::streamFinished);
    connect(&scoreExport_, &ScoreExportRunner::exportFinished,
            this, &MainWindow::scoreExportFinished);
    connect(&scoreRenderer_, &ScoreRenderer::finished,
            this, &MainWindow::scoreRenderFinished);
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

    // A persistent timeline is the live state of the run. It is
    // separate from the results tabs so a later failure cannot erase useful
    // completed-stage evidence.
    installFlatItemViewFactory();
    timeline_ = new QTreeWidget;
    timeline_->setProperty(kFlatItemViewAccessibilityProperty, true);
    timeline_->setColumnCount(4);
    timeline_->setHeaderLabels({tr("Stage"), tr("Rust"), tr("Java"), tr("Comparison")});
    timeline_->setRootIsDecorated(false);
    // Both dynamic item views are flattened at the accessibility boundary. Qt 6.11's Cocoa
    // bridge can otherwise retain a stale accessible row while these models are updated.
    timeline_->setSelectionMode(QAbstractItemView::NoSelection);
    timeline_->setFocusPolicy(Qt::NoFocus);
    timeline_->setItemDelegate(new InspectedRowDelegate(timeline_));
    timeline_->setAccessibleName(tr("Recognition stage timeline"));
    timeline_->setAccessibleDescription(
        tr("Mouse-only visual recognition stage timeline with no accessible row or cell hierarchy. "
           "For keyboard or accessibility use, choose a number in the snapshot stage control and "
           "press Inspect stage."));
    timeline_->setMaximumHeight(205);
    timeline_->viewport()->installEventFilter(this);

    auto *stageInspectorRow = new QHBoxLayout;
    stageIndex_ = new QSpinBox;
    stageIndex_->setRange(1, 1);
    stageIndex_->setEnabled(false);
    stageIndex_->setAccessibleName(tr("Snapshot stage number"));
    stageIndex_->setAccessibleDescription(
        tr("Choose a one-based stage number from the current recognition attempt."));
    inspectStageButton_ = new QPushButton(tr("Inspect stage"));
    inspectStageButton_->setEnabled(false);
    inspectStageButton_->setAccessibleName(tr("Inspect chosen recognition stage"));
    inspectStageButton_->setAccessibleDescription(
        tr("Inspect the numbered stage shown beside this button without entering the visual "
           "timeline."));
    stageInspectorLabel_ = new QLabel(tr("No snapshot stages available."));
    stageInspectorLabel_->setWordWrap(true);
    stageInspectorLabel_->setAccessibleName(tr("Chosen recognition stage status"));
    stageInspectorRow->addWidget(new QLabel(tr("Snapshot stage:")));
    stageInspectorRow->addWidget(stageIndex_);
    stageInspectorRow->addWidget(inspectStageButton_);
    stageInspectorRow->addWidget(stageInspectorLabel_, 1);
    outer->addLayout(stageInspectorRow);
    outer->addWidget(timeline_);

    // --- tabs -----------------------------------------------------------
    tabs_ = new QTabWidget;

    // Page
    pageTab_ = new QWidget;
    auto *pageLayout = new QVBoxLayout(pageTab_);
    auto *toggles = new QHBoxLayout;
    auto *showRust = new QCheckBox(tr("Rust marks"));
    showRust->setChecked(true);
    auto *showJava = new QCheckBox(tr("Java marks"));
    showJava->setChecked(true);
    auto *showRejected = new QCheckBox(tr("Rejected candidates"));
    showRejected->setChecked(true);
    auto *showStaves = new QCheckBox(tr("Staff lines"));
    showStaves->setChecked(true);
    auto *showRelations = new QCheckBox(tr("Graph edges"));
    showRelations->setAccessibleName(tr("Show engine-local graph edges"));
    showRelations->setToolTip(
        tr("Show only relation endpoints whose ids resolve uniquely in the selected engine snapshot."));
    toggles->addWidget(showRust);
    toggles->addWidget(showJava);
    toggles->addWidget(showRejected);
    toggles->addWidget(showStaves);
    toggles->addWidget(showRelations);
    toggles->addStretch(1);
    toggles->addWidget(new QLabel(
        tr("<span style='color:#26a65b'>■</span> agree  "
           "<span style='color:#2980b9'>■</span> Rust only  "
           "<span style='color:#c0392b'>■</span> Java only  "
           "<span style='color:#f39c12'>▨</span> rejected candidate  "
           "<span style='color:#17a2b8'>▣</span> selected Rust  "
           "<span style='color:#9b59b6'>▣</span> selected Java")));
    pageLayout->addLayout(toggles);
    page_ = new PageView;
    pageLayout->addWidget(page_, 1);
    tabs_->addTab(pageTab_, tr("Page"));

    connect(showRust, &QCheckBox::toggled, page_, &PageView::setShowRust);
    connect(showJava, &QCheckBox::toggled, page_, &PageView::setShowJava);
    connect(showRejected, &QCheckBox::toggled, page_, &PageView::setShowRejected);
    connect(showStaves, &QCheckBox::toggled, page_, &PageView::setShowStaves);
    connect(showRelations, &QCheckBox::toggled, page_, &PageView::setShowRelations);

    // Inters
    auto *interTab = new QWidget;
    auto *interLayout = new QVBoxLayout(interTab);
    auto *filterRow = new QHBoxLayout;
    filter_ = new QComboBox;
    filter_->setEditable(true);
    filter_->addItems({QString(), QStringLiteral("BARLINE"), QStringLiteral("BEAM"),
                       QStringLiteral("LEDGER"), QStringLiteral("HEAD"),
                       QStringLiteral("CONNECTOR")});
    auto *showVisiblePairs = new QCheckBox(tr("Highlight visible rows"));
    showVisiblePairs->setAccessibleName(tr("Highlight visible filtered inter rows"));
    showVisiblePairs->setToolTip(
        tr("Draw a translucent Page underlay for every row visible in this filtered table."));
    inspectRow_ = new QSpinBox;
    inspectRow_->setRange(1, 1);
    inspectRow_->setEnabled(false);
    inspectRow_->setAccessibleName(tr("Inter row to inspect"));
    inspectRow_->setAccessibleDescription(
        tr("Choose any visible inter row by its one-based row number."));
    inspectRowButton_ = new QPushButton(tr("Inspect row"));
    inspectRowButton_->setEnabled(false);
    inspectRowButton_->setAccessibleName(tr("Inspect chosen inter row"));
    inspectRowButton_->setAccessibleDescription(
        tr("Inspect the row number chosen beside this button without entering or selecting the "
           "visual table."));
    filterRow->addWidget(new QLabel(tr("Kind contains:")));
    filterRow->addWidget(filter_, 1);
    filterRow->addWidget(showVisiblePairs);
    filterRow->addWidget(new QLabel(tr("Inspect row:")));
    filterRow->addWidget(inspectRow_);
    filterRow->addWidget(inspectRowButton_);
    interLayout->addLayout(filterRow);
    table_ = new QTableWidget;
    table_->setProperty(kFlatItemViewAccessibilityProperty, true);
    table_->setAlternatingRowColors(true);
    table_->setEditTriggers(QAbstractItemView::NoEditTriggers);
    // Qt 6.11's Cocoa bridge crashes while enumerating selected table children on this view.
    // Mouse releases are therefore resolved manually and never create a current index or selection.
    table_->setSelectionMode(QAbstractItemView::NoSelection);
    table_->setFocusPolicy(Qt::NoFocus);
    table_->setItemDelegate(new InspectedRowDelegate(table_));
    table_->setAccessibleName(tr("Same-stage recognition comparison"));
    table_->setAccessibleDescription(
        tr("Visual recognition comparison with no accessible row or cell hierarchy. Click a cell "
           "to inspect its row. For keyboard or accessibility use, choose a row number above and "
           "press Inspect row."));
    table_->viewport()->installEventFilter(this);
    interLayout->addWidget(table_, 1);
    tabs_->addTab(interTab, tr("Inters"));
    connect(filter_, &QComboBox::currentTextChanged, this, &MainWindow::refreshFilter);
    connect(showVisiblePairs, &QCheckBox::toggled, page_, &PageView::setShowVisiblePairs);
    connect(inspectRowButton_, &QPushButton::clicked, this,
            [this] { inspectInterRow(inspectRow_->value() - 1); });

    // Score export is intentionally a separate, explicit PAGE request.  It
    // does not reuse, extend, or wait for the ordinary GRID-through-HEADS
    // streaming run above.
    scoreTab_ = new QWidget;
    auto *scoreLayout = new QVBoxLayout(scoreTab_);
    auto *scoreNotice = new QLabel(tr(
        "<b>Java export and engraving preview.</b> This is not a semantic parity view: "
        "Verovio lays out the Java MusicXML artifact only."));
    scoreNotice->setWordWrap(true);
    scoreNotice->setAccessibleName(tr("Score preview limitation"));
    scoreLayout->addWidget(scoreNotice);

    auto *scoreActions = new QHBoxLayout;
    scoreExportButton_ = new QPushButton(tr("Export & render Java"));
    scoreExportButton_->setAccessibleName(tr("Export and render Java MusicXML"));
    scoreCancelButton_ = new QPushButton(tr("Cancel score request"));
    scoreCancelButton_->setEnabled(false);
    scoreCancelButton_->setAccessibleName(tr("Cancel Java score export or rendering"));
    scoreSaveButton_ = new QPushButton(tr("Save MusicXML…"));
    scoreSaveButton_->setEnabled(false);
    scoreSaveButton_->setAccessibleName(tr("Save Java MusicXML artifact"));
    scoreActions->addWidget(scoreExportButton_);
    scoreActions->addWidget(scoreCancelButton_);
    scoreActions->addWidget(scoreSaveButton_);
    scoreActions->addStretch(1);
    scoreLayout->addLayout(scoreActions);

    scoreActivity_ = new QLabel(tr("No score export requested."));
    scoreActivity_->setWordWrap(true);
    scoreActivity_->setAccessibleName(tr("Score request state"));
    scoreLayout->addWidget(scoreActivity_);

    auto *scorePanels = new QSplitter(Qt::Horizontal);
    auto *javaPanel = new QGroupBox(tr("Java PAGE / MusicXML artifact"));
    auto *javaLayout = new QVBoxLayout(javaPanel);
    scoreDetails_ = new QTextBrowser;
    scoreDetails_->setOpenExternalLinks(false);
    scoreDetails_->setMaximumHeight(130);
    scoreDetails_->setAccessibleName(tr("Java MusicXML artifact details"));
    scoreDetails_->setHtml(tr("<p>Export a sheet explicitly to inspect Java's PAGE/MusicXML artifact.</p>"));
    javaLayout->addWidget(scoreDetails_);
    scoreView_ = new ScoreView;
    scoreView_->setAccessibleName(tr("Rendered Java MusicXML pages"));
    javaLayout->addWidget(scoreView_, 1);

    auto *scoreNavigation = new QHBoxLayout;
    scorePreviousButton_ = new QPushButton(tr("Previous page"));
    scorePreviousButton_->setAccessibleName(tr("Previous rendered score page"));
    scoreNextButton_ = new QPushButton(tr("Next page"));
    scoreNextButton_->setAccessibleName(tr("Next rendered score page"));
    scorePage_ = new QSpinBox;
    scorePage_->setRange(1, 1);
    scorePage_->setPrefix(tr("page "));
    scorePage_->setEnabled(false);
    scorePage_->setAccessibleName(tr("Rendered score page"));
    scoreZoom_ = new QSlider(Qt::Horizontal);
    scoreZoom_->setRange(25, 400);
    scoreZoom_->setValue(100);
    scoreZoom_->setAccessibleName(tr("Rendered score zoom percent"));
    scoreNavigation->addWidget(scorePreviousButton_);
    scoreNavigation->addWidget(scorePage_);
    scoreNavigation->addWidget(scoreNextButton_);
    scoreNavigation->addSpacing(12);
    scoreNavigation->addWidget(new QLabel(tr("Zoom")));
    scoreNavigation->addWidget(scoreZoom_, 1);
    javaLayout->addLayout(scoreNavigation);

    auto *rustPanel = new QGroupBox(tr("Rust score output"));
    auto *rustLayout = new QVBoxLayout(rustPanel);
    auto *rustPlaceholder = new QLabel(tr(
        "<h3>PAGE / MusicXML not implemented</h3>"
        "<p>Rust currently has no PAGE-stage recognition, score assembly, or MusicXML export. "
        "There is therefore no Rust artifact or engraving preview to compare here.</p>"
        "<p>The Java/Verovio image is useful for inspecting Java output, not for claiming "
        "visual or semantic parity.</p>"));
    rustPlaceholder->setWordWrap(true);
    rustPlaceholder->setTextFormat(Qt::RichText);
    rustPlaceholder->setAccessibleName(tr("Rust score output not implemented"));
    rustLayout->addWidget(rustPlaceholder);
    rustLayout->addStretch(1);

    scorePanels->addWidget(javaPanel);
    scorePanels->addWidget(rustPanel);
    scorePanels->setStretchFactor(0, 3);
    scorePanels->setStretchFactor(1, 1);
    scoreLayout->addWidget(scorePanels, 1);
    tabs_->addTab(scoreTab_, tr("Score"));

    connect(scoreExportButton_, &QPushButton::clicked, this, &MainWindow::exportAndRenderJava);
    connect(scoreCancelButton_, &QPushButton::clicked, this, &MainWindow::cancelScore);
    connect(scoreSaveButton_, &QPushButton::clicked, this, &MainWindow::saveMusicXml);
    connect(scorePreviousButton_, &QPushButton::clicked, this, [this] {
        scoreView_->setCurrentPage(scoreView_->currentPage() - 1);
    });
    connect(scoreNextButton_, &QPushButton::clicked, this, [this] {
        scoreView_->setCurrentPage(scoreView_->currentPage() + 1);
    });
    connect(scorePage_, qOverload<int>(&QSpinBox::valueChanged), this, &MainWindow::scorePageChanged);
    connect(scoreZoom_, &QSlider::valueChanged, scoreView_, &ScoreView::setZoomPercent);
    connect(scoreView_, &ScoreView::currentPageChanged,
            this, &MainWindow::scoreViewPageChanged);
    setScoreBusy(false);

    // Status
    status_ = new QTextBrowser;
    status_->setOpenExternalLinks(true);
    tabs_->addTab(status_, tr("Port status"));

    // Log
    log_ = new QPlainTextEdit;
    log_->setReadOnly(true);
    log_->setLineWrapMode(QPlainTextEdit::NoWrap);
    tabs_->addTab(log_, tr("Raw output"));

    outer->addWidget(tabs_, 1);
    setCentralWidget(central);

    connect(runButton_, &QPushButton::clicked, this, &MainWindow::run);
    connect(cancelButton_, &QPushButton::clicked, this, &MainWindow::cancel);
    connect(stageIndex_, qOverload<int>(&QSpinBox::valueChanged), this,
            [this] { updateStageInspectorLabel(); });
    connect(inspectStageButton_, &QPushButton::clicked, this, [this] {
        const int index = stageIndex_->value() - 1;
        if (index >= 0 && index < stageChoices_.size()) {
            selectStage(stageChoices_[index], true);
        }
    });
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

void MainWindow::setScoreBusy(bool busy, const QString &status)
{
    scoreExportButton_->setEnabled(!busy);
    scoreCancelButton_->setEnabled(busy);
    scoreSaveButton_->setEnabled(!busy && scoreArtifact_.success && !scoreArtifact_.path.isEmpty());
    const bool pages = !busy && scoreView_->pageCount() > 0;
    scorePage_->setEnabled(pages);
    scorePreviousButton_->setEnabled(pages && scoreView_->currentPage() > 0);
    scoreNextButton_->setEnabled(pages && scoreView_->currentPage() + 1 < scoreView_->pageCount());
    scoreZoom_->setEnabled(pages);
    if (!status.isEmpty()) {
        scoreActivity_->setText(status);
    }
}

void MainWindow::showScoreExport(const ScoreExportResult &result)
{
    scoreDetails_->setHtml(tr(
        "<p><b>Java PAGE export complete.</b></p>"
        "<p><b>Artifact:</b> <code>%1</code><br>"
        "<b>Format:</b> %2 &nbsp; <b>Bytes:</b> %3<br>"
        "<b>SHA-256:</b> <code>%4</code><br>"
        "<b>Java recognition:</b> %5 ms</p>"
        "<p>Rendering with Verovio…</p>")
                                   .arg(result.path.toHtmlEscaped(), result.format.toHtmlEscaped())
                                   .arg(result.bytes)
                                   .arg(result.sha256.toHtmlEscaped())
                                   .arg(QString::number(result.elapsedMillis, 'f', 1)));
}

void MainWindow::showScoreRender(const ScoreRenderResult &result)
{
    scoreDetails_->setHtml(tr(
        "<p><b>Java PAGE export and Verovio render complete.</b></p>"
        "<p><b>Artifact:</b> <code>%1</code><br>"
        "<b>Format:</b> %2 &nbsp; <b>Bytes:</b> %3<br>"
        "<b>SHA-256:</b> <code>%4</code><br>"
        "<b>Renderer:</b> %5 &mdash; <code>%6</code><br>"
        "<b>Executable:</b> <code>%7</code><br>"
        "<b>Rendered pages:</b> %8</p>"
        "<p><i>Engraving is a Java artifact preview, not a semantic parity result.</i></p>")
                                   .arg(scoreArtifact_.path.toHtmlEscaped())
                                   .arg(scoreArtifact_.format.toHtmlEscaped())
                                   .arg(scoreArtifact_.bytes)
                                   .arg(scoreArtifact_.sha256.toHtmlEscaped())
                                   .arg(result.renderer.toHtmlEscaped())
                                   .arg(result.rendererVersion.toHtmlEscaped())
                                   .arg(result.rendererExecutable.toHtmlEscaped())
                                   .arg(result.pagePaths.size()));
}

void MainWindow::showScoreFailure(const QString &title, const QString &message,
                                  const QString &diagnostics)
{
    QString details = tr("<p><b>%1</b></p><p>%2</p>")
                          .arg(title.toHtmlEscaped(), message.toHtmlEscaped());
    if (!diagnostics.isEmpty()) {
        details += tr("<p><b>Diagnostics</b></p><pre>%1</pre>")
                       .arg(diagnostics.toHtmlEscaped());
    }
    if (scoreArtifact_.success && !scoreArtifact_.path.isEmpty()) {
        details += tr(
            "<p><b>Java MusicXML artifact remains available.</b><br>"
            "<b>Artifact:</b> <code>%1</code><br>"
            "<b>Format:</b> %2 &nbsp; <b>Bytes:</b> %3<br>"
            "<b>SHA-256:</b> <code>%4</code></p>")
                       .arg(scoreArtifact_.path.toHtmlEscaped())
                       .arg(scoreArtifact_.format.toHtmlEscaped())
                       .arg(scoreArtifact_.bytes)
                       .arg(scoreArtifact_.sha256.toHtmlEscaped());
    }
    scoreDetails_->setHtml(details);
    scoreActivity_->setText(tr("Score request failed: %1").arg(message));
}

void MainWindow::exportAndRenderJava()
{
    if (scoreState_ != ScoreState::Idle) {
        showScoreFailure(tr("Score request already active"),
                         tr("Wait for or cancel the active export/render request first."));
        return;
    }

    // The prior QTemporaryDir is intentionally kept until this explicit next
    // score request. Never delete it while either child process may still be
    // reading/writing it.
    scoreAttempt_ = std::make_unique<QTemporaryDir>();
    if (!scoreAttempt_->isValid()) {
        scoreAttempt_.reset();
        showScoreFailure(tr("Could not prepare score export"),
                         tr("Could not create a private temporary directory for this score request."));
        return;
    }
    scoreArtifact_ = {};
    scoreView_->clearPages();
    scoreZoom_->setValue(100);
    setScoreBusy(true, tr("Starting explicit Java PAGE export…"));

    ScoreExportResult preflight;
    const QString output = scoreAttempt_->filePath(QStringLiteral("java-page.mxl"));
    if (!scoreExport_.startJava(input_->currentText(), sheet_->value(), output, &preflight)) {
        scoreState_ = ScoreState::Idle;
        showScoreFailure(tr("Java PAGE export could not start"), preflight.message,
                         preflight.diagnostics);
        setScoreBusy(false);
        return;
    }
    scoreState_ = ScoreState::Exporting;
    scoreActivity_->setText(tr("Java is recognizing PAGE and exporting MusicXML…"));
}

void MainWindow::cancelScore()
{
    if (scoreState_ == ScoreState::Exporting) {
        scoreActivity_->setText(tr("Cancelling Java PAGE export; waiting for the process to exit…"));
        scoreCancelButton_->setEnabled(false);
        scoreExport_.cancel();
    } else if (scoreState_ == ScoreState::Rendering) {
        scoreActivity_->setText(tr("Cancelling Verovio render; waiting for the process to exit…"));
        scoreCancelButton_->setEnabled(false);
        scoreRenderer_.cancel();
    }
}

void MainWindow::scoreExportFinished(ScoreExportResult result)
{
    if (scoreState_ != ScoreState::Exporting) {
        return;
    }
    if (!result.success) {
        showScoreFailure(tr("Java PAGE export failed"),
                         tr("%1: %2").arg(result.errorCode, result.message), result.diagnostics);
        finishScoreAttempt();
        return;
    }

    scoreArtifact_ = result;
    showScoreExport(result);
    scoreState_ = ScoreState::Rendering;
    scoreActivity_->setText(tr("Rendering Java MusicXML with Verovio…"));
    QString error;
    if (!scoreRenderer_.start({result.path, scoreAttempt_->path(), 120'000}, &error)) {
        showScoreFailure(tr("Verovio renderer unavailable"), error);
        finishScoreAttempt();
    }
}

void MainWindow::scoreRenderFinished(const ScoreRenderResult &result)
{
    if (scoreState_ != ScoreState::Rendering) {
        return;
    }
    if (!result.succeeded()) {
        showScoreFailure(result.cancelled ? tr("Verovio render cancelled")
                                          : tr("Verovio render failed"),
                         result.error);
        finishScoreAttempt();
        return;
    }
    scoreView_->setPages(result.pagePaths);
    scoreZoom_->setValue(100);
    showScoreRender(result);
    scoreActivity_->setText(tr("Java PAGE artifact rendered with Verovio."));
    finishScoreAttempt();
}

void MainWindow::scorePageChanged(int page)
{
    scoreView_->setCurrentPage(page - 1);
}

void MainWindow::scoreViewPageChanged(int page, int pageCount)
{
    const QSignalBlocker block(scorePage_);
    scorePage_->setRange(1, std::max(1, pageCount));
    if (page >= 0) {
        scorePage_->setValue(page + 1);
    }
    setScoreBusy(scoreState_ != ScoreState::Idle);
}

void MainWindow::saveMusicXml()
{
    const QFileInfo source(scoreArtifact_.path);
    if (!scoreArtifact_.success || !source.isFile()) {
        showScoreFailure(tr("No Java MusicXML artifact to save"),
                         tr("Export and render Java successfully before saving the artifact."));
        return;
    }
    const QString filter = scoreArtifact_.format == QLatin1String("mxl")
        ? tr("Compressed MusicXML (*.mxl)") : tr("MusicXML (*.xml)");
    const QString destination = QFileDialog::getSaveFileName(
        this, tr("Save Java MusicXML artifact"),
        QDir::home().filePath(source.fileName()), filter);
    if (destination.isEmpty()) {
        return;
    }
    QFile input(source.absoluteFilePath());
    QSaveFile output(destination);
    if (!input.open(QIODevice::ReadOnly) || !output.open(QIODevice::WriteOnly)) {
        showScoreFailure(tr("Could not save Java MusicXML artifact"),
                         tr("Could not open the selected destination for writing."));
        return;
    }
    while (!input.atEnd()) {
        const QByteArray chunk = input.read(64 * 1024);
        if (chunk.isEmpty() || output.write(chunk) != chunk.size()) {
            output.cancelWriting();
            showScoreFailure(tr("Could not save Java MusicXML artifact"),
                             tr("Writing the selected destination failed."));
            return;
        }
    }
    if (!output.commit()) {
        showScoreFailure(tr("Could not save Java MusicXML artifact"), output.errorString());
        return;
    }
    scoreActivity_->setText(tr("Saved Java MusicXML artifact to %1").arg(destination));
}

void MainWindow::finishScoreAttempt()
{
    scoreState_ = ScoreState::Idle;
    setScoreBusy(false);
    closeWhenScoreIdle();
}

void MainWindow::closeWhenScoreIdle()
{
    if (!closeWhenScoreIdle_ || scoreState_ != ScoreState::Idle) {
        return;
    }
    closeWhenScoreIdle_ = false;
    QTimer::singleShot(0, this, &QWidget::close);
}

void MainWindow::closeEvent(QCloseEvent *event)
{
    if (scoreState_ == ScoreState::Idle) {
        QMainWindow::closeEvent(event);
        return;
    }
    closeWhenScoreIdle_ = true;
    cancelScore();
    event->ignore();
}

bool MainWindow::eventFilter(QObject *watched, QEvent *event)
{
    const bool interViewport = table_ && watched == table_->viewport();
    const bool timelineViewport = timeline_ && watched == timeline_->viewport();
    if (!interViewport && !timelineViewport) {
        return QMainWindow::eventFilter(watched, event);
    }

    if (event->type() == QEvent::MouseMove) {
        const auto *mouse = static_cast<QMouseEvent *>(event);
        if (mouse->buttons() != Qt::NoButton) {
            event->accept();
            return true;
        }
    }
    if (event->type() == QEvent::MouseButtonPress
        || event->type() == QEvent::MouseButtonDblClick) {
        event->accept();
        return true;
    }
    if (event->type() != QEvent::MouseButtonRelease) {
        return QMainWindow::eventFilter(watched, event);
    }

    const auto *mouse = static_cast<QMouseEvent *>(event);
    if (mouse->button() == Qt::LeftButton) {
        const QPoint position = mouse->position().toPoint();
        if (interViewport) {
            const QModelIndex index = table_->indexAt(position);
            if (index.isValid()) {
                inspectInterRow(index.row());
            }
        } else if (QTreeWidgetItem *item = timeline_->itemAt(position)) {
            selectStage(item->data(0, Qt::UserRole).toString(), true);
        }
    }
    event->accept();
    return true;
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
    refreshStageInspector();
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
    page_->setVisiblePairs({});
    page_->setSelectedPairing(std::nullopt);
    visiblePairs_.clear();
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
    refreshStageInspector();
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
        for (int column = 0; column < timeline_->columnCount(); ++column) {
            row->setData(column, kInspectedRole, stage == selectedStage_);
        }
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

void MainWindow::refreshStageInspector()
{
    QStringList stages;
    for (const QString &stage : streamStages()) {
        if (stageSnapshots_.contains(stage)) {
            stages << stage;
        }
    }

    const QString priorChoice = stageIndex_->isEnabled()
        && stageIndex_->value() - 1 < stageChoices_.size()
        ? stageChoices_[stageIndex_->value() - 1]
        : QString();
    const bool choicesChanged = stageChoices_ != stages;
    stageChoices_ = stages;

    {
        const QSignalBlocker block(stageIndex_);
        stageIndex_->setRange(1, std::max(1, static_cast<int>(stages.size())));
        stageIndex_->setEnabled(!stages.isEmpty());
        if (choicesChanged && !stages.isEmpty()) {
            int desired = stages.indexOf(priorChoice);
            if (desired < 0) {
                desired = stages.indexOf(selectedStage_);
            }
            stageIndex_->setValue(std::max(0, desired) + 1);
        }
    }
    inspectStageButton_->setEnabled(!stages.isEmpty());
    updateStageInspectorLabel();
}

void MainWindow::updateStageInspectorLabel()
{
    const int index = stageIndex_->value() - 1;
    if (index < 0 || index >= stageChoices_.size()) {
        stageInspectorLabel_->setText(tr("No snapshot stages available."));
        stageInspectorLabel_->setAccessibleDescription(
            tr("No recognition stage snapshot is available to inspect."));
        return;
    }
    const QString &stage = stageChoices_[index];
    const StagePair &pair = stageSnapshots_[stage];
    const QString status = tr("%1 of %2: %3 — Rust: %4; Java: %5")
                               .arg(index + 1)
                               .arg(stageChoices_.size())
                               .arg(stage, stageCell(pair.rust), stageCell(pair.java));
    stageInspectorLabel_->setText(status);
    stageInspectorLabel_->setAccessibleDescription(status);
}

void MainWindow::selectStage(const QString &stage, bool fromUser)
{
    if (!stageSnapshots_.contains(stage)) {
        return;
    }
    if (fromUser) {
        followLatest_ = false;
    }
    const QString priorStage = selectedStage_;
    selectedStage_ = stage;
    {
        const int index = stageChoices_.indexOf(stage);
        if (index >= 0) {
            const QSignalBlocker block(stageIndex_);
            stageIndex_->setValue(index + 1);
        }
    }
    updateStageInspectorLabel();
    for (const QString &markedStage : {priorStage, selectedStage_}) {
        if (QTreeWidgetItem *row = timelineRows_.value(markedStage, nullptr)) {
            const bool inspected = markedStage == selectedStage_;
            for (int column = 0; column < timeline_->columnCount(); ++column) {
                row->setData(column, kInspectedRole, inspected);
            }
        }
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
    visiblePairs_ = rows;
    inspectedRow_ = -1;
    {
        const QSignalBlocker block(inspectRow_);
        inspectRow_->setRange(1, std::max(1, static_cast<int>(rows.size())));
        inspectRow_->setValue(1);
        inspectRow_->setEnabled(!rows.isEmpty());
    }
    inspectRowButton_->setEnabled(!rows.isEmpty());
    table_->setAccessibleDescription(
        tr("Visual recognition comparison with no accessible row or cell hierarchy. Click a cell "
           "to inspect its row. For keyboard or accessibility use, choose a row number above and "
           "press Inspect row."));
    page_->setVisiblePairs(visiblePairs_);
    // Filtering changes the table's visible set. Do not leave an old selected
    // row outlined when it is no longer one of the rows the user can inspect.
    page_->setSelectedPairing(std::nullopt);

    const QStringList headers{tr("Kind"),        tr("Staff"),      tr("Rust x"),
                              tr("Java x"),      tr("Rust grade"), tr("Java grade"),
                              tr("Δ grade"),     tr("Rust impacts"), tr("Java impacts"),
                              tr("Note")};
    const QSignalBlocker block(table_);
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
            item->setData(Qt::UserRole, row);
            item->setData(kInspectedRole, false);
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

void MainWindow::inspectInterRow(int row)
{
    const auto markInspected = [this](int markedRow, bool inspected) {
        if (markedRow < 0 || markedRow >= table_->rowCount()) {
            return;
        }
        for (int column = 0; column < table_->columnCount(); ++column) {
            if (QTableWidgetItem *item = table_->item(markedRow, column)) {
                item->setData(kInspectedRole, inspected);
            }
        }
    };

    if (row < 0 || row >= visiblePairs_.size()) {
        markInspected(inspectedRow_, false);
        inspectedRow_ = -1;
        page_->setSelectedPairing(std::nullopt);
        return;
    }
    // `visiblePairs_` is rebuilt from the same filtered, same-stage rows that
    // the table renders. The selection therefore cannot re-pair ids or reach
    // into a later streaming snapshot.
    const auto *item = table_->item(row, 0);
    if (!item || item->data(Qt::UserRole).toInt() != row) {
        markInspected(inspectedRow_, false);
        inspectedRow_ = -1;
        page_->setSelectedPairing(std::nullopt);
        return;
    }

    if (inspectedRow_ != row) {
        markInspected(inspectedRow_, false);
        inspectedRow_ = row;
        markInspected(inspectedRow_, true);
    }
    {
        const QSignalBlocker block(inspectRow_);
        inspectRow_->setValue(row + 1);
    }
    page_->setSelectedPairing(visiblePairs_[row]);
    table_->setAccessibleDescription(
        tr("Inspecting row %1 of %2: %3. Open the Page tab to view its centred overlay. "
           "This inspector does not use table selection.")
            .arg(row + 1)
            .arg(table_->rowCount())
            .arg(item->text()));
}

} // namespace omrscope
