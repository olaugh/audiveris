// SPDX-License-Identifier: AGPL-3.0-or-later
#include "MainWindow.h"
#include "PageView.h"
#include "ScoreView.h"

#include <QAccessible>
#include <QComboBox>
#include <QItemSelectionModel>
#include <QLabel>
#include <QPushButton>
#include <QSpinBox>
#include <QTabBar>
#include <QTableWidget>
#include <QTabWidget>
#include <QTextBrowser>
#include <QTreeWidget>
#include <QWidget>
#include <QtTest>

namespace omrscope {

namespace {

Inter inter(int id, int system, int staff, const QRectF &bounds, const QString &kind)
{
    Inter value;
    value.id = id;
    value.system = system;
    value.staff = staff;
    value.kind = kind;
    value.bounds = bounds;
    return value;
}

} // namespace

class MainWindowTest : public QObject
{
    Q_OBJECT

private slots:
    void filteredTableInspectionUpdatesPageWithoutLeavingInters();
    void dynamicItemViewsUseFlatAccessibilityAndIndependentChoosers();
    void scoreTabIsExplicitAndRustOutputStaysUnavailable();
};

void MainWindowTest::filteredTableInspectionUpdatesPageWithoutLeavingInters()
{
    MainWindow window{QDir(QDir::tempPath())};
    window.resize(1000, 700);

    window.rust_.image = QSize(1000, 700);
    window.rust_.inters << inter(1, 2, 3, QRectF(720, 300, 20, 30), QStringLiteral("BEAM"));
    window.rust_.inters << inter(2, 2, 3, QRectF(80, 100, 10, 30), QStringLiteral("BEAM"));
    window.java_.image = window.rust_.image;
    window.java_.inters << inter(11, 2, 3, QRectF(721, 300, 20, 30), QStringLiteral("BEAM"));
    window.java_.inters << inter(12, 2, 3, QRectF(80, 100, 10, 30), QStringLiteral("BEAM"));
    window.selectedStage_ = QStringLiteral("BEAMS");
    window.showResults();

    window.filter_->setCurrentText(QStringLiteral("BEAM"));
    QCOMPARE(window.table_->rowCount(), 2);
    QCOMPARE(window.visiblePairs_.size(), 2);
    QVERIFY(window.visiblePairs_[0].rust.has_value());
    QVERIFY(window.visiblePairs_[0].java.has_value());

    const int interTab = window.tabs_->indexOf(window.table_->parentWidget());
    const int pageTab = window.tabs_->indexOf(window.pageTab_);
    QVERIFY(interTab >= 0);
    QVERIFY(pageTab >= 0);
    window.show();
    QTRY_VERIFY(window.isVisible());
    window.tabs_->setCurrentIndex(interTab);
    QSignalSpy tabChanges(window.tabs_, &QTabWidget::currentChanged);
    QCOMPARE(window.table_->selectionMode(), QAbstractItemView::NoSelection);
    QCOMPARE(window.table_->focusPolicy(), Qt::NoFocus);
    QVERIFY(!window.table_->currentIndex().isValid());
    QVERIFY(window.table_->selectionModel()->selectedIndexes().isEmpty());
    QSignalSpy tableCurrentChanges(window.table_->selectionModel(),
                                   &QItemSelectionModel::currentChanged);
    QVERIFY(window.inspectRow_->isEnabled());
    QCOMPARE(window.inspectRow_->minimum(), 1);
    QCOMPARE(window.inspectRow_->maximum(), 2);
    QVERIFY(window.inspectRowButton_->isEnabled());

    QAccessibleInterface *accessible = QAccessible::queryAccessibleInterface(window.table_);
    QVERIFY(accessible);
    QCOMPARE(accessible->role(), QAccessible::Client);
    QCOMPARE(accessible->childCount(), 0);
    QCOMPARE(accessible->text(QAccessible::Name), window.table_->accessibleName());
    QVERIFY(!accessible->selectionInterface());
    QVERIFY(!accessible->tableInterface());
    QVERIFY(!accessible->tableCellInterface());

    // The independent, accessible row chooser covers every visible row without touching the
    // table's accessibility hierarchy or its native selection model.
    const QVector<QPointF> expectedFocus{QPointF(730.5, 315), QPointF(85, 115)};
    for (int row = 0; row < window.table_->rowCount(); ++row) {
        window.inspectRow_->setValue(row + 1);
        QTest::mouseClick(window.inspectRowButton_, Qt::LeftButton);
        QCOMPARE(window.inspectedRow_, row);
        QCOMPARE(window.tabs_->currentIndex(), interTab);
        QCOMPARE(window.page_->selectedFocus(), std::optional<QPointF>(expectedFocus[row]));
        QVERIFY(window.table_->selectionModel()->selectedIndexes().isEmpty());
        QVERIFY(window.table_->selectedItems().isEmpty());
        QVERIFY(!accessible->selectionInterface());
        QVERIFY(!accessible->tableInterface());
        for (int candidate = 0; candidate < window.table_->rowCount(); ++candidate) {
            for (int column = 0; column < window.table_->columnCount(); ++column) {
                QCOMPARE(window.table_->item(candidate, column)
                             ->data(Qt::UserRole + 1)
                             .toBool(),
                         candidate == row);
            }
        }
    }
    QCOMPARE(tabChanges.count(), 0);

    const QRect firstCell = window.table_->visualItemRect(window.table_->item(0, 0));
    QVERIFY(firstCell.isValid());
    QTest::mouseClick(window.table_->viewport(), Qt::LeftButton, Qt::NoModifier,
                      firstCell.center());
    // A real cell click updates the hidden Page model and the separate inspector role, while the
    // Qt selection model exposed to Cocoa accessibility remains empty.
    QTest::qWait(20);
    QCOMPARE(tabChanges.count(), 0);
    QCOMPARE(window.tabs_->currentIndex(), interTab);
    QCOMPARE(window.inspectedRow_, 0);
    QCOMPARE(window.inspectRow_->value(), 1);
    QVERIFY(!window.table_->hasFocus());
    QVERIFY(!window.table_->currentIndex().isValid());
    QCOMPARE(tableCurrentChanges.count(), 0);
    QVERIFY(window.table_->selectionModel()->selectedIndexes().isEmpty());
    QVERIFY(window.table_->selectedItems().isEmpty());
    QVERIFY(!accessible->selectionInterface());
    QVERIFY(!accessible->tableInterface());
    QVERIFY(window.table_->selectionModel()->selectedIndexes().isEmpty());
    for (int column = 0; column < window.table_->columnCount(); ++column) {
        QVERIFY(window.table_->item(0, column)->data(Qt::UserRole + 1).toBool());
        QVERIFY(!window.table_->item(1, column)->data(Qt::UserRole + 1).toBool());
    }
    QVERIFY(window.table_->accessibleDescription().contains(QStringLiteral("row 1 of 2")));
    const auto focus = window.page_->selectedFocus();
    QVERIFY(focus.has_value());
    QCOMPARE(*focus, QPointF(730.5, 315));

    // Keyboard users reach the same row through the selection-free numeric chooser and button.
    window.inspectRow_->setValue(2);
    window.inspectRowButton_->setFocus();
    QTest::keyClick(window.inspectRowButton_, Qt::Key_Space);
    QCOMPARE(window.inspectedRow_, 1);
    QCOMPARE(window.inspectRow_->value(), 2);
    QVERIFY(!window.table_->hasFocus());
    QVERIFY(!window.table_->currentIndex().isValid());
    QVERIFY(window.table_->selectionModel()->selectedIndexes().isEmpty());
    QVERIFY(!accessible->selectionInterface());
    for (int column = 0; column < window.table_->columnCount(); ++column) {
        QVERIFY(!window.table_->item(0, column)->data(Qt::UserRole + 1).toBool());
        QVERIFY(window.table_->item(1, column)->data(Qt::UserRole + 1).toBool());
    }
    QCOMPARE(window.page_->selectedFocus(), std::optional<QPointF>(QPointF(85, 115)));

    // Page becomes visible only through an explicit tab-bar click, after which the staged
    // selection and centering remain intact.
    QTest::mouseClick(window.tabs_->tabBar(), Qt::LeftButton, Qt::NoModifier,
                      window.tabs_->tabBar()->tabRect(pageTab).center());
    QCOMPARE(window.tabs_->currentWidget(), window.pageTab_);
    QCOMPARE(tabChanges.count(), 1);
    QCOMPARE(window.page_->selectedFocus(), std::optional<QPointF>(QPointF(85, 115)));
}

void MainWindowTest::dynamicItemViewsUseFlatAccessibilityAndIndependentChoosers()
{
    MainWindow window{QDir(QDir::tempPath())};
    window.resize(1000, 700);

    QCOMPARE(window.timeline_->selectionMode(), QAbstractItemView::NoSelection);
    QCOMPARE(window.timeline_->focusPolicy(), Qt::NoFocus);
    QVERIFY(!window.timeline_->currentIndex().isValid());
    QVERIFY(window.timeline_->selectionModel()->selectedIndexes().isEmpty());
    QAccessibleInterface *timelineAccessible =
        QAccessible::queryAccessibleInterface(window.timeline_);
    QVERIFY(timelineAccessible);
    QCOMPARE(timelineAccessible->role(), QAccessible::Client);
    QCOMPARE(timelineAccessible->childCount(), 0);
    QCOMPARE(timelineAccessible->text(QAccessible::Name), window.timeline_->accessibleName());
    QVERIFY(!timelineAccessible->selectionInterface());
    QVERIFY(!timelineAccessible->tableInterface());
    QVERIFY(!timelineAccessible->tableCellInterface());
    QAccessibleInterface *stageNumberAccessible =
        QAccessible::queryAccessibleInterface(window.stageIndex_);
    QVERIFY(stageNumberAccessible);
    QCOMPARE(stageNumberAccessible->role(), QAccessible::SpinBox);
    QVERIFY(!stageNumberAccessible->tableInterface());

    MainWindow::StagePair grid;
    grid.rust.state = StageState::Completed;
    grid.java.state = StageState::Completed;
    grid.rust.result.ran = true;
    grid.java.result.ran = true;
    MainWindow::StagePair beams = grid;
    beams.rust.result.inters
        << inter(31, 1, 1, QRectF(20, 30, 10, 12), QStringLiteral("BEAM"));
    beams.java.result.inters
        << inter(41, 1, 1, QRectF(20, 30, 10, 12), QStringLiteral("BEAM"));
    window.stageSnapshots_.insert(QStringLiteral("GRID"), grid);
    window.stageSnapshots_.insert(QStringLiteral("BEAMS"), beams);
    window.updateTimeline();
    QSignalSpy timelineCurrentChanges(window.timeline_->selectionModel(),
                                      &QItemSelectionModel::currentChanged);

    QVERIFY(window.stageIndex_->isEnabled());
    QCOMPARE(window.stageIndex_->minimum(), 1);
    QCOMPARE(window.stageIndex_->maximum(), 2);
    QVERIFY(window.inspectStageButton_->isEnabled());
    QCOMPARE(window.stageChoices_, QStringList({QStringLiteral("GRID"),
                                                QStringLiteral("BEAMS")}));
    QCOMPARE(window.timeline_->topLevelItemCount(), 2);

    // The numeric chooser and static button reach every snapshot while the flattened visual
    // timeline never exposes or creates selected row/cell interfaces.
    for (int index = 0; index < window.stageChoices_.size(); ++index) {
        window.stageIndex_->setValue(index + 1);
        QVERIFY(window.stageInspectorLabel_->text().contains(window.stageChoices_[index]));
        QTest::mouseClick(window.inspectStageButton_, Qt::LeftButton);
        QCOMPARE(window.selectedStage_, window.stageChoices_[index]);
        QVERIFY(!window.timeline_->currentIndex().isValid());
        QVERIFY(window.timeline_->selectionModel()->selectedIndexes().isEmpty());
        QVERIFY(window.timeline_->selectedItems().isEmpty());
        QVERIFY(!timelineAccessible->selectionInterface());
        QVERIFY(!timelineAccessible->tableInterface());
        for (int candidate = 0; candidate < window.timeline_->topLevelItemCount(); ++candidate) {
            for (int column = 0; column < window.timeline_->columnCount(); ++column) {
                QCOMPARE(window.timeline_->topLevelItem(candidate)
                             ->data(column, Qt::UserRole + 1)
                             .toBool(),
                         candidate == index);
            }
        }
    }

    window.show();
    QTRY_VERIFY(window.isVisible());
    const QRect firstStage = window.timeline_->visualItemRect(window.timeline_->topLevelItem(0));
    QVERIFY(firstStage.isValid());
    QTest::mouseClick(window.timeline_->viewport(), Qt::LeftButton, Qt::NoModifier,
                      firstStage.center());
    QCOMPARE(window.selectedStage_, QStringLiteral("GRID"));
    QCOMPARE(window.stageIndex_->value(), 1);
    QVERIFY(!window.timeline_->hasFocus());
    QVERIFY(!window.timeline_->currentIndex().isValid());
    QCOMPARE(timelineCurrentChanges.count(), 0);
    QVERIFY(window.timeline_->selectionModel()->selectedIndexes().isEmpty());

    window.stageIndex_->setValue(2);
    window.inspectStageButton_->setFocus();
    QTest::keyClick(window.inspectStageButton_, Qt::Key_Space);
    QCOMPARE(window.selectedStage_, QStringLiteral("BEAMS"));
    QCOMPARE(window.stageIndex_->value(), 2);
    QVERIFY(!window.timeline_->currentIndex().isValid());
    QVERIFY(window.timeline_->selectionModel()->selectedIndexes().isEmpty());
    QVERIFY(window.timeline_->selectedItems().isEmpty());
    QCOMPARE(timelineAccessible->childCount(), 0);

    // The factory is property-scoped and remains correct after a second window installs/uses it.
    MainWindow second{QDir(QDir::tempPath())};
    QAccessibleInterface *secondTimeline =
        QAccessible::queryAccessibleInterface(second.timeline_);
    QAccessibleInterface *secondTable = QAccessible::queryAccessibleInterface(second.table_);
    QAccessibleInterface *ordinaryButton =
        QAccessible::queryAccessibleInterface(second.runButton_);
    QVERIFY(secondTimeline);
    QVERIFY(secondTable);
    QVERIFY(ordinaryButton);
    QCOMPARE(secondTimeline->childCount(), 0);
    QCOMPARE(secondTable->childCount(), 0);
    QVERIFY(!secondTimeline->selectionInterface());
    QVERIFY(!secondTable->selectionInterface());
    QCOMPARE(ordinaryButton->role(), QAccessible::Button);

    QTableWidget ordinaryTable(1, 1);
    ordinaryTable.setItem(0, 0, new QTableWidgetItem(QStringLiteral("ordinary")));
    QTreeWidget ordinaryTree;
    ordinaryTree.setColumnCount(1);
    auto *ordinaryTreeRow = new QTreeWidgetItem(&ordinaryTree);
    ordinaryTreeRow->setText(0, QStringLiteral("ordinary"));
    QAccessibleInterface *ordinaryTableAccessible =
        QAccessible::queryAccessibleInterface(&ordinaryTable);
    QAccessibleInterface *ordinaryTreeAccessible =
        QAccessible::queryAccessibleInterface(&ordinaryTree);
    QVERIFY(ordinaryTableAccessible);
    QVERIFY(ordinaryTreeAccessible);
    QVERIFY(ordinaryTableAccessible->selectionInterface());
    QVERIFY(ordinaryTableAccessible->tableInterface());
    QVERIFY(ordinaryTreeAccessible->selectionInterface());
    QVERIFY(ordinaryTreeAccessible->tableInterface());
}

void MainWindowTest::scoreTabIsExplicitAndRustOutputStaysUnavailable()
{
    MainWindow window{QDir(QDir::tempPath())};
    QCOMPARE(window.scoreState_, MainWindow::ScoreState::Idle);
    QVERIFY(!window.scoreAttempt_);
    QVERIFY(window.scoreExportButton_->isEnabled());
    QVERIFY(!window.scoreCancelButton_->isEnabled());
    QVERIFY(!window.scorePreviousButton_->isEnabled());
    QVERIFY(!window.scoreNextButton_->isEnabled());
    QCOMPARE(window.scoreView_->pageCount(), 0);
    QCOMPARE(window.tabs_->tabText(window.tabs_->indexOf(window.scoreTab_)), QStringLiteral("Score"));

    bool sawRustUnavailable = false;
    for (const QLabel *label : window.scoreTab_->findChildren<QLabel *>()) {
        if (label->accessibleName() == QLatin1String("Rust score output not implemented")) {
            sawRustUnavailable = label->text().contains(QStringLiteral("PAGE / MusicXML not implemented"));
        }
    }
    QVERIFY(sawRustUnavailable);

    window.scoreArtifact_.success = true;
    window.scoreArtifact_.path = QStringLiteral("/tmp/java-page.mxl");
    window.scoreArtifact_.format = QStringLiteral("mxl");
    window.scoreArtifact_.bytes = 42;
    window.scoreArtifact_.sha256 = QString(64, QLatin1Char('a'));
    ScoreRenderResult render;
    render.renderer = QStringLiteral("Verovio");
    render.rendererVersion = QStringLiteral("Verovio test");
    render.rendererExecutable = QStringLiteral("/tmp/<b>literal path</b>&verovio");
    render.pagePaths = {QStringLiteral("page.svg")};
    window.showScoreRender(render);
    const QString renderedDetails = window.scoreDetails_->toPlainText();
    QVERIFY(renderedDetails.contains(QStringLiteral("Executable:")));
    QVERIFY(renderedDetails.contains(render.rendererExecutable));

    window.scoreArtifact_.path = QStringLiteral("/tmp/<b>literal artifact</b>&.mxl");
    window.showScoreFailure(QStringLiteral("Renderer failed"),
                            QStringLiteral("synthetic render failure"));
    const QString failedRenderDetails = window.scoreDetails_->toPlainText();
    QVERIFY(failedRenderDetails.contains(
        QStringLiteral("Java MusicXML artifact remains available.")));
    QVERIFY(failedRenderDetails.contains(window.scoreArtifact_.path));
    QVERIFY(failedRenderDetails.contains(window.scoreArtifact_.sha256));

    // Selecting the ordinary HEADS snapshot is a display action only. It does
    // not turn the concurrent recognition stream into a PAGE export request.
    window.selectedStage_ = QStringLiteral("HEADS");
    window.showResults();
    QCOMPARE(window.scoreState_, MainWindow::ScoreState::Idle);
    QVERIFY(!window.scoreAttempt_);

    // A score attempt exists only after the dedicated user action. The empty
    // test repository is rejected synchronously and cannot leave an export
    // child running or present a Rust engraving as a fallback.
    window.exportAndRenderJava();
    QCOMPARE(window.scoreState_, MainWindow::ScoreState::Idle);
    QVERIFY(window.scoreAttempt_);
    QVERIFY(!window.scoreExport_.isRunning());
    QVERIFY(window.scoreDetails_->toPlainText().contains(QStringLiteral("could not start"),
                                                         Qt::CaseInsensitive));
}

} // namespace omrscope

QTEST_MAIN(omrscope::MainWindowTest)

#include "MainWindowTest.moc"
