// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include "EngineRunner.h"
#include "Model.h"

#include <QDir>
#include <QHash>
#include <QMainWindow>

class QCheckBox;
class QComboBox;
class QLabel;
class QPlainTextEdit;
class QProgressBar;
class QPushButton;
class QSpinBox;
class QTableWidget;
class QTextBrowser;
class QTreeWidget;
class QTreeWidgetItem;

namespace omrscope {

class PageView;

/// The window: pick a sheet, run both engines, see what each made of it.
///
/// Four tabs, which are the four questions this tool exists to answer --
/// what does the page look like to each engine, where do they differ, how
/// much of the pipeline is ported, and what did the engines actually say.
class MainWindow : public QMainWindow
{
    Q_OBJECT

public:
    explicit MainWindow(QDir repository, QWidget *parent = nullptr);

private slots:
    void run();
    void cancel();
    void refreshFilter();
    void stageEvent(omrscope::Engine engine, omrscope::StreamEvent event,
                    omrscope::EngineResult result);
    void streamFinished(omrscope::Engine engine, bool success, bool cancelled,
                        const QString &message);
    void stageSelected();

private:
    void buildUi();
    void setBusy(bool busy, const QString &what = {});
    void loadInputs();
    void loadStatus();
    void showResults();
    void updateTimeline();
    void selectStage(const QString &stage, bool fromUser = false);
    void followNewestCommonStage();
    void setStageState(Engine engine, const StreamEvent &event, const EngineResult &result);
    QString terminalSummaryHtml() const;
    QImage renderInput(const QString &input, int sheet) const;

    struct StagePair {
        StageSnapshot rust;
        StageSnapshot java;
        QString comparison;
        bool comparisonReady = false;
    };

    QDir repository_;
    EngineRunner runner_;
    EngineResult rust_;
    EngineResult java_;

    QComboBox *input_ = nullptr;
    QSpinBox *sheet_ = nullptr;
    QComboBox *step_ = nullptr;
    QCheckBox *withRust_ = nullptr;
    QCheckBox *withJava_ = nullptr;
    QComboBox *filter_ = nullptr;

    PageView *page_ = nullptr;
    QTableWidget *table_ = nullptr;
    QTextBrowser *status_ = nullptr;
    QPlainTextEdit *log_ = nullptr;
    QLabel *summary_ = nullptr;
    QProgressBar *progress_ = nullptr;
    QPushButton *runButton_ = nullptr;
    QPushButton *cancelButton_ = nullptr;
    QTreeWidget *timeline_ = nullptr;
    QHash<QString, QTreeWidgetItem *> timelineRows_;

    QHash<QString, StagePair> stageSnapshots_;
    QString pendingInput_;
    int pendingSheet_ = 1;
    QString pendingStep_;
    QString selectedStage_;
    bool followLatest_ = true;
    bool selectingProgrammatically_ = false;
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
