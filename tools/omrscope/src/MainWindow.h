// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include "EngineRunner.h"
#include "Model.h"
#include "ScoreExportRunner.h"
#include "ScoreRenderer.h"

#include <QDir>
#include <QHash>
#include <QMainWindow>
#include <QStringList>
#include <QTemporaryDir>

#include <memory>

class QCheckBox;
class QComboBox;
class QCloseEvent;
class QEvent;
class QLabel;
class QPlainTextEdit;
class QProgressBar;
class QPushButton;
class QSlider;
class QSpinBox;
class QTableWidget;
class QTextBrowser;
class QTabWidget;
class QTreeWidget;
class QTreeWidgetItem;
class QWidget;

namespace omrscope {

class PageView;
class ScoreView;

/// The window: pick a sheet, run both engines, see what each made of it.
///
/// Five tabs answer the tool's inspection questions: what each engine drew on
/// the page, where their inters differ, what Java exported as MusicXML, how
/// much of the pipeline is ported, and what the engines actually reported.
class MainWindow : public QMainWindow
{
    Q_OBJECT

    friend class MainWindowTest;

public:
    explicit MainWindow(QDir repository, QWidget *parent = nullptr);

protected:
    void closeEvent(QCloseEvent *event) override;
    bool eventFilter(QObject *watched, QEvent *event) override;

private slots:
    void run();
    void cancel();
    void refreshFilter();
    void exportAndRenderJava();
    void cancelScore();
    void scoreExportFinished(omrscope::ScoreExportResult result);
    void scoreRenderFinished(const omrscope::ScoreRenderResult &result);
    void scorePageChanged(int page);
    void scoreViewPageChanged(int page, int pageCount);
    void saveMusicXml();
    void stageEvent(omrscope::Engine engine, omrscope::StreamEvent event,
                    omrscope::EngineResult result);
    void streamFinished(omrscope::Engine engine, bool success, bool cancelled,
                        const QString &message);

private:
    void buildUi();
    void setBusy(bool busy, const QString &what = {});
    void loadInputs();
    void loadStatus();
    void showResults();
    void updateTimeline();
    void refreshStageInspector();
    void updateStageInspectorLabel();
    void selectStage(const QString &stage, bool fromUser = false);
    void followNewestCommonStage();
    void setStageState(Engine engine, const StreamEvent &event, const EngineResult &result);
    QString terminalSummaryHtml() const;
    QImage renderInput(const QString &input, int sheet) const;
    void setScoreBusy(bool busy, const QString &status = {});
    void showScoreExport(const ScoreExportResult &result);
    void showScoreRender(const ScoreRenderResult &result);
    void showScoreFailure(const QString &title, const QString &message,
                          const QString &diagnostics = {});
    void finishScoreAttempt();
    void closeWhenScoreIdle();
    void inspectInterRow(int row);

    struct StagePair {
        StageSnapshot rust;
        StageSnapshot java;
        QString comparison;
        bool comparisonReady = false;
    };

    QDir repository_;
    EngineRunner runner_;
    ScoreExportRunner scoreExport_;
    ScoreRenderer scoreRenderer_;
    EngineResult rust_;
    EngineResult java_;

    QComboBox *input_ = nullptr;
    QSpinBox *sheet_ = nullptr;
    QComboBox *step_ = nullptr;
    QCheckBox *withRust_ = nullptr;
    QCheckBox *withJava_ = nullptr;
    QComboBox *filter_ = nullptr;

    PageView *page_ = nullptr;
    QWidget *scoreTab_ = nullptr;
    ScoreView *scoreView_ = nullptr;
    QTabWidget *tabs_ = nullptr;
    QWidget *pageTab_ = nullptr;
    QTableWidget *table_ = nullptr;
    QTextBrowser *status_ = nullptr;
    QPlainTextEdit *log_ = nullptr;
    QLabel *summary_ = nullptr;
    QProgressBar *progress_ = nullptr;
    QPushButton *runButton_ = nullptr;
    QPushButton *cancelButton_ = nullptr;
    QSpinBox *inspectRow_ = nullptr;
    QPushButton *inspectRowButton_ = nullptr;
    QPushButton *scoreExportButton_ = nullptr;
    QPushButton *scoreCancelButton_ = nullptr;
    QPushButton *scoreSaveButton_ = nullptr;
    QPushButton *scorePreviousButton_ = nullptr;
    QPushButton *scoreNextButton_ = nullptr;
    QSpinBox *scorePage_ = nullptr;
    QSlider *scoreZoom_ = nullptr;
    QLabel *scoreActivity_ = nullptr;
    QTextBrowser *scoreDetails_ = nullptr;
    QTreeWidget *timeline_ = nullptr;
    QSpinBox *stageIndex_ = nullptr;
    QLabel *stageInspectorLabel_ = nullptr;
    QPushButton *inspectStageButton_ = nullptr;
    QHash<QString, QTreeWidgetItem *> timelineRows_;
    QStringList stageChoices_;
    QVector<Pairing> visiblePairs_;
    int inspectedRow_ = -1;
    std::unique_ptr<QTemporaryDir> scoreAttempt_;
    ScoreExportResult scoreArtifact_;

    enum class ScoreState { Idle, Exporting, Rendering };
    ScoreState scoreState_ = ScoreState::Idle;
    bool closeWhenScoreIdle_ = false;

    QHash<QString, StagePair> stageSnapshots_;
    QString pendingInput_;
    int pendingSheet_ = 1;
    QString pendingStep_;
    QString selectedStage_;
    bool followLatest_ = true;
    bool rustRequested_ = false;
    bool javaRequested_ = false;
    bool rustFinished_ = false;
    bool javaFinished_ = false;
    bool rustSucceeded_ = false;
    bool javaSucceeded_ = false;
    bool cancellationRequested_ = false;
    QString rustTerminalMessage_;
    QString javaTerminalMessage_;
};

} // namespace omrscope
