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

find_package(Qt6 REQUIRED COMPONENTS Widgets Pdf Concurrent)

include(CTest)

add_executable(omrscope
    src/main.cpp
    src/MainWindow.cpp
    src/PageView.cpp
    src/Parsers.cpp
    src/EngineRunner.cpp
)

target_link_libraries(omrscope PRIVATE Qt6::Widgets Qt6::Pdf Qt6::Concurrent)
target_compile_options(omrscope PRIVATE -Wall -Wextra)

if(BUILD_TESTING)
    add_executable(omrscope-parser-tests
        tests/ParsersTest.cpp
        src/Parsers.cpp
    )
    target_include_directories(omrscope-parser-tests PRIVATE src)
    target_link_libraries(omrscope-parser-tests PRIVATE Qt6::Widgets)
    target_compile_options(omrscope-parser-tests PRIVATE -Wall -Wextra)
    add_test(NAME omrscope-parser-tests COMMAND omrscope-parser-tests)
endif()
