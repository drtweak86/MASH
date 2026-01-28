#include "mainwindow.h"
#include "ui_mainwindow.h"
#include <QStandardPaths>
#include <QDir>
#include <QDateTime>
#include <QScrollBar>

MainWindow::MainWindow(QWidget *parent)
    : QMainWindow(parent)
    , ui(new Ui::MainWindow)
    , installerProcess(nullptr)
    , progressTimer(new QTimer(this))
    , progressValue(0)
{
    ui->setupUi(this);
    
    // Set window properties
    setWindowTitle("MASH Installer - Fedora KDE for Raspberry Pi 4");
    setMinimumSize(800, 600);
    
    // Connect signals
    connect(progressTimer, &QTimer::timeout, this, &MainWindow::updateProgress);
    
    // Initialize UI
    ui->progressBar->setValue(0);
    ui->progressBar->setVisible(false);
    loadDisks();
    
    // Set default UEFI directory
    ui->lineEditUEFI->setText("/boot/efi");
    
    appendLog("🚀 MASH Installer ready", "blue");
    appendLog("⚠️  WARNING: This will ERASE the selected disk!", "red");
}

MainWindow::~MainWindow()
{
    if (installerProcess && installerProcess->state() == QProcess::Running) {
        installerProcess->kill();
        installerProcess->waitForFinished();
    }
    delete ui;
}

void MainWindow::on_btnBrowseImage_clicked()
{
    QString fileName = QFileDialog::getOpenFileName(
        this,
        "Select Fedora KDE Image",
        QDir::homePath(),
        "Disk Images (*.raw *.img *.iso);;All Files (*)"
    );
    
    if (!fileName.isEmpty()) {
        ui->lineEditImage->setText(fileName);
        appendLog("Selected image: " + fileName, "green");
    }
}

void MainWindow::on_btnRefreshDisks_clicked()
{
    loadDisks();
    appendLog("Disk list refreshed", "blue");
}

void MainWindow::on_btnBrowseUEFI_clicked()
{
    QString dirName = QFileDialog::getExistingDirectory(
        this,
        "Select UEFI Directory",
        ui->lineEditUEFI->text()
    );
    
    if (!dirName.isEmpty()) {
        ui->lineEditUEFI->setText(dirName);
        appendLog("UEFI directory: " + dirName, "green");
    }
}

void MainWindow::on_btnInstall_clicked()
{
    if (!validateInputs()) {
        return;
    }
    
    // Confirm destructive operation
    QString disk = getSelectedDisk();
    QMessageBox::StandardButton reply = QMessageBox::warning(
        this,
        "Confirm Installation",
        QString("⚠️  THIS WILL COMPLETELY ERASE %1!\n\n"
                "All data on this disk will be PERMANENTLY DELETED.\n\n"
                "Partition layout:\n"
                "  • EFI:   512 MB\n"
                "  • BOOT:  1 GB\n"
                "  • ROOT:  1.8 TB\n"
                "  • DATA:  Remaining space\n\n"
                "Are you ABSOLUTELY SURE?").arg(disk),
        QMessageBox::Yes | QMessageBox::No,
        QMessageBox::No
    );
    
    if (reply != QMessageBox::Yes) {
        appendLog("Installation cancelled by user", "orange");
        return;
    }
    
    // Double confirmation
    reply = QMessageBox::critical(
        this,
        "FINAL WARNING",
        QString("Last chance! Clicking YES will START ERASING %1!\n\n"
                "This CANNOT be undone!").arg(disk),
        QMessageBox::Yes | QMessageBox::No,
        QMessageBox::No
    );
    
    if (reply != QMessageBox::Yes) {
        appendLog("Installation cancelled by user", "orange");
        return;
    }
    
    // Start installation
    appendLog("========================================", "blue");
    appendLog("🔥 STARTING INSTALLATION", "red");
    appendLog("========================================", "blue");
    
    setUIEnabled(false);
    ui->progressBar->setVisible(true);
    ui->progressBar->setValue(0);
    progressValue = 0;
    progressTimer->start(500);
    
    // Build command
    QStringList args;
    args << "flash";
    args << "--image" << ui->lineEditImage->text();
    args << "--disk" << disk;
    args << "--uefi-dir" << ui->lineEditUEFI->text();
    args << "--auto-unmount";
    args << "--yes-i-know";
    
    if (ui->checkBoxDryRun->isChecked()) {
        args << "--dry-run";
        appendLog("🧪 DRY RUN MODE - No changes will be made", "orange");
    }
    
    // Create process
    installerProcess = new QProcess(this);
    connect(installerProcess, &QProcess::readyReadStandardOutput, 
            this, &MainWindow::onProcessOutput);
    connect(installerProcess, &QProcess::readyReadStandardError, 
            this, &MainWindow::onProcessError);
    connect(installerProcess, QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished),
            this, &MainWindow::onProcessFinished);
    
    // Find mash-installer binary
    QString installerPath = "./target/release/mash-installer";
    if (!QFile::exists(installerPath)) {
        installerPath = "mash-installer";  // Try PATH
    }
    
    appendLog("Command: " + installerPath + " " + args.join(" "), "gray");
    
    // Start with sudo
    QStringList sudoArgs;
    sudoArgs << installerPath << args;
    installerProcess->start("pkexec", sudoArgs);
    
    if (!installerProcess->waitForStarted()) {
        appendLog("❌ Failed to start installer!", "red");
        setUIEnabled(true);
        ui->progressBar->setVisible(false);
        progressTimer->stop();
    }
}

void MainWindow::on_btnCancel_clicked()
{
    if (installerProcess && installerProcess->state() == QProcess::Running) {
        QMessageBox::StandardButton reply = QMessageBox::question(
            this,
            "Cancel Installation",
            "Are you sure you want to cancel the installation?\n"
            "This may leave your disk in an inconsistent state!",
            QMessageBox::Yes | QMessageBox::No
        );
        
        if (reply == QMessageBox::Yes) {
            appendLog("⚠️  Cancelling installation...", "orange");
            installerProcess->kill();
        }
    }
}

void MainWindow::onProcessOutput()
{
    if (installerProcess) {
        QString output = installerProcess->readAllStandardOutput();
        appendLog(output.trimmed(), "black");
    }
}

void MainWindow::onProcessError()
{
    if (installerProcess) {
        QString error = installerProcess->readAllStandardError();
        appendLog(error.trimmed(), "red");
    }
}

void MainWindow::onProcessFinished(int exitCode, QProcess::ExitStatus exitStatus)
{
    progressTimer->stop();
    ui->progressBar->setValue(100);
    
    if (exitStatus == QProcess::NormalExit && exitCode == 0) {
        appendLog("========================================", "green");
        appendLog("✅ INSTALLATION COMPLETE!", "green");
        appendLog("========================================", "green");
        appendLog("", "black");
        appendLog("Next steps:", "blue");
        appendLog("  1. Safely eject the SD card/USB drive", "black");
        appendLog("  2. Insert into Raspberry Pi 4", "black");
        appendLog("  3. Ensure UEFI firmware is installed (not U-Boot)", "black");
        appendLog("  4. Power on and enjoy Fedora KDE!", "black");
        
        QMessageBox::information(
            this,
            "Installation Complete",
            "✅ Installation successful!\n\n"
            "You can now safely eject the drive and boot your Raspberry Pi 4."
        );
    } else {
        appendLog("========================================", "red");
        appendLog("❌ INSTALLATION FAILED!", "red");
        appendLog("========================================", "red");
        appendLog(QString("Exit code: %1").arg(exitCode), "red");
        
        QMessageBox::critical(
            this,
            "Installation Failed",
            QString("❌ Installation failed with exit code %1\n\n"
                    "Check the log for details.").arg(exitCode)
        );
    }
    
    setUIEnabled(true);
    delete installerProcess;
    installerProcess = nullptr;
}

void MainWindow::updateProgress()
{
    progressValue = (progressValue + 1) % 100;
    ui->progressBar->setValue(progressValue);
}

void MainWindow::loadDisks()
{
    ui->comboBoxDisk->clear();
    
    QProcess lsblk;
    lsblk.start("lsblk", QStringList() << "-d" << "-n" << "-o" << "NAME,SIZE,MODEL");
    lsblk.waitForFinished();
    
    QString output = lsblk.readAllStandardOutput();
    QStringList lines = output.split('\n', Qt::SkipEmptyParts);
    
    for (const QString &line : lines) {
        QStringList parts = line.split(QRegularExpression("\\s+"), Qt::SkipEmptyParts);
        if (parts.size() >= 2) {
            QString name = parts[0];
            // Skip loop devices and small devices
            if (!name.startsWith("loop") && !name.startsWith("ram")) {
                QString size = parts[1];
                QString model = parts.size() > 2 ? parts.mid(2).join(" ") : "Unknown";
                ui->comboBoxDisk->addItem(
                    QString("%1 (%2) - %3").arg(name).arg(size).arg(model),
                    name
                );
            }
        }
    }
    
    if (ui->comboBoxDisk->count() == 0) {
        ui->comboBoxDisk->addItem("No disks found", "");
    }
}

void MainWindow::setUIEnabled(bool enabled)
{
    ui->lineEditImage->setEnabled(enabled);
    ui->btnBrowseImage->setEnabled(enabled);
    ui->comboBoxDisk->setEnabled(enabled);
    ui->btnRefreshDisks->setEnabled(enabled);
    ui->lineEditUEFI->setEnabled(enabled);
    ui->btnBrowseUEFI->setEnabled(enabled);
    ui->checkBoxDryRun->setEnabled(enabled);
    ui->btnInstall->setEnabled(enabled);
    ui->btnCancel->setEnabled(!enabled);
}

void MainWindow::appendLog(const QString &text, const QString &color)
{
    QString timestamp = QDateTime::currentDateTime().toString("[HH:mm:ss]");
    QString html = QString("<span style='color:%1'>%2 %3</span><br>")
                   .arg(color)
                   .arg(timestamp)
                   .arg(text.toHtmlEscaped());
    
    ui->textEditLog->append(html);
    
    // Auto-scroll to bottom
    QScrollBar *scrollBar = ui->textEditLog->verticalScrollBar();
    scrollBar->setValue(scrollBar->maximum());
}

QString MainWindow::getSelectedDisk()
{
    return ui->comboBoxDisk->currentData().toString();
}

bool MainWindow::validateInputs()
{
    if (ui->lineEditImage->text().isEmpty()) {
        QMessageBox::warning(this, "Missing Input", "Please select a disk image file.");
        return false;
    }
    
    if (!QFile::exists(ui->lineEditImage->text())) {
        QMessageBox::warning(this, "Invalid Input", "The selected image file does not exist.");
        return false;
    }
    
    QString disk = getSelectedDisk();
    if (disk.isEmpty()) {
        QMessageBox::warning(this, "Missing Input", "Please select a target disk.");
        return false;
    }
    
    if (ui->lineEditUEFI->text().isEmpty()) {
        QMessageBox::warning(this, "Missing Input", "Please specify the UEFI directory.");
        return false;
    }
    
    return true;
}
