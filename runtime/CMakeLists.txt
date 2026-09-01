# ─────────────────────────────────────────────────────────────────────────────
# Hefaos Runtime - Root CMake Configuration
# ─────────────────────────────────────────────────────────────────────────────

cmake_minimum_required(VERSION 3.25)

project(hefaos-runtime
    VERSION 0.1.0
    DESCRIPTION "Hefaos Real-time Robotics Runtime"
    LANGUAGES CXX
)

# ─────────────────────────────────────────────────────────────────────────────
# CMake Configuration
# ─────────────────────────────────────────────────────────────────────────────

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Default to Release build
if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(CMAKE_BUILD_TYPE Release CACHE STRING "Build type" FORCE)
endif()

# CMake modules
list(APPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}/cmake")

# ─────────────────────────────────────────────────────────────────────────────
# Options
# ─────────────────────────────────────────────────────────────────────────────

option(BUILD_TESTING "Build tests" ON)
option(BUILD_BENCHMARKS "Build benchmarks" OFF)
option(BUILD_DOCS "Build documentation" OFF)
option(ENABLE_COVERAGE "Enable code coverage" OFF)
option(ENABLE_SANITIZERS "Enable address and UB sanitizers" OFF)
option(ENABLE_CLANG_TIDY "Enable clang-tidy" OFF)

# Component options
option(HEFAOS_BUILD_CORE "Build core library" ON)
option(HEFAOS_BUILD_HAL "Build HAL library" ON)
option(HEFAOS_BUILD_AI "Build AI runtime" ON)
option(HEFAOS_BUILD_ROS2_BRIDGE "Build ROS2 bridge" OFF)

# ─────────────────────────────────────────────────────────────────────────────
# Compiler Configuration
# ─────────────────────────────────────────────────────────────────────────────

# Compiler warnings
add_compile_options(
    -Wall
    -Wextra
    -Wpedantic
    -Werror
    -Wno-unused-parameter
)

# Optimization for Release
if(CMAKE_BUILD_TYPE STREQUAL "Release")
    add_compile_options(-O3 -march=native)
endif()

# Debug flags
if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    add_compile_options(-g -O0)
endif()

# Sanitizers
if(ENABLE_SANITIZERS)
    add_compile_options(-fsanitize=address,undefined -fno-omit-frame-pointer)
    add_link_options(-fsanitize=address,undefined)
endif()

# Coverage
if(ENABLE_COVERAGE)
    add_compile_options(--coverage -fprofile-arcs -ftest-coverage)
    add_link_options(--coverage)
endif()

# Clang-tidy
if(ENABLE_CLANG_TIDY)
    find_program(CLANG_TIDY_EXE NAMES clang-tidy clang-tidy-18)
    if(CLANG_TIDY_EXE)
        set(CMAKE_CXX_CLANG_TIDY ${CLANG_TIDY_EXE})
    endif()
endif()

# ─────────────────────────────────────────────────────────────────────────────
# Dependencies
# ─────────────────────────────────────────────────────────────────────────────

# Required dependencies
find_package(Threads REQUIRED)

# iceoryx for zero-copy IPC
find_package(iceoryx_posh CONFIG)
find_package(iceoryx_hoofs CONFIG)

# FlatBuffers for serialization
find_package(FlatBuffers)

# Apache Arrow for columnar data
find_package(Arrow)

# TBB for parallel algorithms
find_package(TBB)

# spdlog for logging
find_package(spdlog)

# yaml-cpp for configuration
find_package(yaml-cpp)

# fmt for string formatting
find_package(fmt)

# Testing dependencies
if(BUILD_TESTING)
    enable_testing()
    find_package(GTest)
endif()

# ─────────────────────────────────────────────────────────────────────────────
# Subdirectories
# ─────────────────────────────────────────────────────────────────────────────

if(HEFAOS_BUILD_CORE)
    add_subdirectory(core)
endif()

if(HEFAOS_BUILD_HAL)
    add_subdirectory(hal)
endif()

if(HEFAOS_BUILD_AI)
    add_subdirectory(ai)
endif()

if(HEFAOS_BUILD_ROS2_BRIDGE)
    add_subdirectory(ros2-bridge)
endif()

# ─────────────────────────────────────────────────────────────────────────────
# Installation
# ─────────────────────────────────────────────────────────────────────────────

include(GNUInstallDirs)
include(CMakePackageConfigHelpers)

# Generate version file
write_basic_package_version_file(
    "${CMAKE_CURRENT_BINARY_DIR}/HefaosConfigVersion.cmake"
    VERSION ${PROJECT_VERSION}
    COMPATIBILITY SameMajorVersion
)

# Install CMake config
install(
    FILES
        "${CMAKE_CURRENT_SOURCE_DIR}/cmake/HefaosConfig.cmake"
        "${CMAKE_CURRENT_BINARY_DIR}/HefaosConfigVersion.cmake"
    DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/Hefaos
)

# ─────────────────────────────────────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────────────────────────────────────

message(STATUS "")
message(STATUS "Hefaos Runtime Configuration:")
message(STATUS "  Version:        ${PROJECT_VERSION}")
message(STATUS "  Build type:     ${CMAKE_BUILD_TYPE}")
message(STATUS "  C++ Standard:   ${CMAKE_CXX_STANDARD}")
message(STATUS "  Compiler:       ${CMAKE_CXX_COMPILER_ID} ${CMAKE_CXX_COMPILER_VERSION}")
message(STATUS "")
message(STATUS "Components:")
message(STATUS "  Core:           ${HEFAOS_BUILD_CORE}")
message(STATUS "  HAL:            ${HEFAOS_BUILD_HAL}")
message(STATUS "  AI:             ${HEFAOS_BUILD_AI}")
message(STATUS "  ROS2 Bridge:    ${HEFAOS_BUILD_ROS2_BRIDGE}")
message(STATUS "")
message(STATUS "Options:")
message(STATUS "  Testing:        ${BUILD_TESTING}")
message(STATUS "  Benchmarks:     ${BUILD_BENCHMARKS}")
message(STATUS "  Coverage:       ${ENABLE_COVERAGE}")
message(STATUS "  Sanitizers:     ${ENABLE_SANITIZERS}")
message(STATUS "  Clang-tidy:     ${ENABLE_CLANG_TIDY}")
message(STATUS "")
