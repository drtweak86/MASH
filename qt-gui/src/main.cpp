#include <QApplication>
#include "mainwindow.h"

int main(int argc, char *argv[])
{
    QApplication app(argc, argv);
    app.setApplicationName("MASH Installer");
    app.setApplicationVersion("0.3.0");
    app.setOrganizationName("MASH");
    
    MainWindow window;
    window.show();
    
    return app.exec();
}
