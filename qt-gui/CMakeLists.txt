cmake_minimum_required(VERSION 3.16)

project(mash-installer-qt VERSION 0.3.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

find_package(Qt6 COMPONENTS Core Widgets QUIET)
if (NOT Qt6_FOUND)
    find_package(Qt5 5.15 REQUIRED COMPONENTS Core Widgets)
endif()

set(PROJECT_SOURCES
    src/main.cpp
    src/mainwindow.cpp
    src/mainwindow.h
    src/mainwindow.ui
)

if(Qt6_FOUND)
    qt_add_executable(mash-installer-qt ${PROJECT_SOURCES})
else()
    add_executable(mash-installer-qt ${PROJECT_SOURCES})
endif()

if(Qt6_FOUND)
    target_link_libraries(mash-installer-qt PRIVATE Qt6::Core Qt6::Widgets)
else()
    target_link_libraries(mash-installer-qt PRIVATE Qt5::Core Qt5::Widgets)
endif()

# Install target
install(TARGETS mash-installer-qt
    RUNTIME DESTINATION bin
)

# Desktop file
configure_file(
    ${CMAKE_CURRENT_SOURCE_DIR}/mash-installer.desktop.in
    ${CMAKE_CURRENT_BINARY_DIR}/mash-installer.desktop
    @ONLY
)

install(FILES ${CMAKE_CURRENT_BINARY_DIR}/mash-installer.desktop
    DESTINATION share/applications
)
