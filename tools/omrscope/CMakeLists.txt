# SPDX-License-Identifier: AGPL-3.0-or-later
cmake_minimum_required(VERSION 3.21)
project(omrscope LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

# Homebrew's Qt is not on the default search path.
if(APPLE AND NOT DEFINED CMAKE_PREFIX_PATH)
    execute_process(COMMAND brew --prefix qt
                    OUTPUT_VARIABLE HOMEBREW_QT
                    OUTPUT_STRIP_TRAILING_WHITESPACE
                    ERROR_QUIET)
    if(HOMEBREW_QT)
        list(APPEND CMAKE_PREFIX_PATH "${HOMEBREW_QT}")
    endif()
endif()

find_package(Qt6 REQUIRED COMPONENTS Widgets Pdf SvgWidgets)

include(CTest)

add_executable(omrscope
    src/main.cpp
    src/MainWindow.cpp
    src/PageView.cpp
    src/Parsers.cpp
    src/EngineRunner.cpp
    src/Model.cpp
    src/ScoreRenderer.cpp
    src/ScoreExportRunner.cpp
    src/ScoreView.cpp
)

target_link_libraries(omrscope PRIVATE Qt6::Widgets Qt6::Pdf Qt6::SvgWidgets)
target_compile_options(omrscope PRIVATE -Wall -Wextra)

if(BUILD_TESTING)
    find_package(Qt6 REQUIRED COMPONENTS Test)

    add_executable(omrscope-parser-tests
        tests/ParsersTest.cpp
        src/Parsers.cpp
    )
    target_include_directories(omrscope-parser-tests PRIVATE src)
    target_link_libraries(omrscope-parser-tests PRIVATE Qt6::Widgets)
    target_compile_options(omrscope-parser-tests PRIVATE -Wall -Wextra)
    add_test(NAME omrscope-parser-tests COMMAND omrscope-parser-tests)

    add_executable(omrscope-runner-tests
        tests/EngineRunnerTest.cpp
        src/EngineRunner.cpp
        src/Parsers.cpp
    )
    target_include_directories(omrscope-runner-tests PRIVATE src)
    target_link_libraries(omrscope-runner-tests PRIVATE Qt6::Widgets)
    target_compile_options(omrscope-runner-tests PRIVATE -Wall -Wextra)
    add_test(NAME omrscope-runner-tests COMMAND omrscope-runner-tests)

    add_executable(omrscope-page-view-tests
        tests/PageViewTest.cpp
        src/PageView.cpp
        src/Parsers.cpp
        src/Model.cpp
    )
    target_include_directories(omrscope-page-view-tests PRIVATE src)
    target_link_libraries(omrscope-page-view-tests PRIVATE Qt6::Widgets Qt6::Test)
    target_compile_options(omrscope-page-view-tests PRIVATE -Wall -Wextra)
    add_test(NAME omrscope-page-view-tests COMMAND omrscope-page-view-tests)

    add_executable(omrscope-main-window-tests
        tests/MainWindowTest.cpp
        src/MainWindow.cpp
        src/PageView.cpp
        src/Parsers.cpp
        src/EngineRunner.cpp
        src/Model.cpp
        src/ScoreRenderer.cpp
        src/ScoreExportRunner.cpp
        src/ScoreView.cpp
    )
    target_include_directories(omrscope-main-window-tests PRIVATE src)
    target_link_libraries(omrscope-main-window-tests PRIVATE Qt6::Widgets Qt6::Pdf Qt6::SvgWidgets Qt6::Test)
    target_compile_options(omrscope-main-window-tests PRIVATE -Wall -Wextra)
    add_test(NAME omrscope-main-window-tests COMMAND omrscope-main-window-tests)

    add_executable(omrscope-score-renderer-tests
        tests/ScoreRendererTest.cpp
        src/ScoreRenderer.cpp
    )
    target_include_directories(omrscope-score-renderer-tests PRIVATE src)
    target_link_libraries(omrscope-score-renderer-tests PRIVATE Qt6::Core)
    target_compile_options(omrscope-score-renderer-tests PRIVATE -Wall -Wextra)
    add_test(NAME omrscope-score-renderer-tests COMMAND omrscope-score-renderer-tests)

    add_executable(omrscope-score-export-runner-tests
        tests/ScoreExportRunnerTest.cpp
        src/ScoreExportRunner.cpp
    )
    target_include_directories(omrscope-score-export-runner-tests PRIVATE src)
    target_link_libraries(omrscope-score-export-runner-tests PRIVATE Qt6::Core)
    target_compile_options(omrscope-score-export-runner-tests PRIVATE -Wall -Wextra)
    add_test(NAME omrscope-score-export-runner-tests COMMAND omrscope-score-export-runner-tests)

    add_executable(omrscope-score-view-tests
        tests/ScoreViewTest.cpp
        src/ScoreView.cpp
    )
    target_include_directories(omrscope-score-view-tests PRIVATE src)
    target_link_libraries(omrscope-score-view-tests PRIVATE Qt6::Widgets Qt6::SvgWidgets Qt6::Test)
    target_compile_options(omrscope-score-view-tests PRIVATE -Wall -Wextra)
    add_test(NAME omrscope-score-view-tests COMMAND omrscope-score-view-tests)

    set_tests_properties(omrscope-page-view-tests omrscope-main-window-tests omrscope-score-view-tests
                         PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen")
endif()
