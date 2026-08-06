// SPDX-License-Identifier: AGPL-3.0-or-later
#pragma once

#include "EngineRunner.h"
#include "Model.h"

#include <QDir>
#include <QMainWindow>

class QCheckBox;
class QComboBox;
class QLabel;
class QPlainTextEdit;
class QSpinBox;
class QTableWidget;
class QTextBrowser;

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
    void refreshFilter();

private:
    void buildUi();
    void loadInputs();
    void loadStatus();
    void showResults();
    QImage renderInput(const QString &input, int sheet) const;

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
};

} // namespace omrscope
