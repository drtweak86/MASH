#ifndef MAINWINDOW_H
#define MAINWINDOW_H

#include <QMainWindow>
#include <QProcess>
#include <QString>
#include <QFileDialog>
#include <QMessageBox>
#include <QTimer>

QT_BEGIN_NAMESPACE
namespace Ui { class MainWindow; }
QT_END_NAMESPACE

class MainWindow : public QMainWindow
{
    Q_OBJECT

public:
    MainWindow(QWidget *parent = nullptr);
    ~MainWindow();

private slots:
    void on_btnBrowseImage_clicked();
    void on_btnRefreshDisks_clicked();
    void on_btnBrowseUEFI_clicked();
    void on_btnInstall_clicked();
    void on_btnCancel_clicked();
    void onProcessOutput();
    void onProcessError();
    void onProcessFinished(int exitCode, QProcess::ExitStatus exitStatus);
    void updateProgress();

private:
    Ui::MainWindow *ui;
    QProcess *installerProcess;
    QTimer *progressTimer;
    int progressValue;
    
    void loadDisks();
    void setUIEnabled(bool enabled);
    void appendLog(const QString &text, const QString &color = "black");
    QString getSelectedDisk();
    bool validateInputs();
};

#endif // MAINWINDOW_H
