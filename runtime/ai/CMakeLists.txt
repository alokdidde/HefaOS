# ─────────────────────────────────────────────────────────────────────────────
# Hefaos AI Runtime
# ─────────────────────────────────────────────────────────────────────────────
#
# AI components:
#   - Model loading and management
#   - Inference backends (ONNX Runtime, TensorFlow Lite, llama.cpp)
#   - Perception pipeline
#   - Model optimization utilities
#
# ─────────────────────────────────────────────────────────────────────────────

add_library(hefaos-ai)
add_library(Hefaos::ai ALIAS hefaos-ai)

target_sources(hefaos-ai
    PRIVATE
        # Source files (to be implemented)
        src/runtime.cpp
        src/model.cpp
        src/model_loader.cpp
        src/inference_session.cpp
        # Backends
        src/backends/onnx_backend.cpp
        src/backends/tflite_backend.cpp
        src/backends/llama_backend.cpp
)

target_include_directories(hefaos-ai
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
        $<INSTALL_INTERFACE:include>
)

target_compile_features(hefaos-ai PUBLIC cxx_std_20)

# Dependencies
target_link_libraries(hefaos-ai
    PUBLIC
        Hefaos::core
        Threads::Threads
)

# Optional dependencies
if(TARGET spdlog::spdlog)
    target_link_libraries(hefaos-ai PUBLIC spdlog::spdlog)
endif()

# ─────────────────────────────────────────────────────────────────────────────
# ONNX Runtime Backend
# ─────────────────────────────────────────────────────────────────────────────

find_package(onnxruntime QUIET)
if(onnxruntime_FOUND)
    target_link_libraries(hefaos-ai PUBLIC onnxruntime::onnxruntime)
    target_compile_definitions(hefaos-ai PUBLIC HEFAOS_HAS_ONNX)
    message(STATUS "ONNX Runtime: Found")
else()
    message(STATUS "ONNX Runtime: Not found (ONNX backend disabled)")
endif()

# ─────────────────────────────────────────────────────────────────────────────
# TensorFlow Lite Backend
# ─────────────────────────────────────────────────────────────────────────────

find_package(TensorFlowLite QUIET)
if(TensorFlowLite_FOUND)
    target_link_libraries(hefaos-ai PUBLIC TensorFlowLite::TensorFlowLite)
    target_compile_definitions(hefaos-ai PUBLIC HEFAOS_HAS_TFLITE)
    message(STATUS "TensorFlow Lite: Found")
else()
    message(STATUS "TensorFlow Lite: Not found (TFLite backend disabled)")
endif()

# ─────────────────────────────────────────────────────────────────────────────
# llama.cpp Backend (for LLM inference)
# ─────────────────────────────────────────────────────────────────────────────

find_package(llama QUIET)
if(llama_FOUND)
    target_link_libraries(hefaos-ai PUBLIC llama::llama)
    target_compile_definitions(hefaos-ai PUBLIC HEFAOS_HAS_LLAMA)
    message(STATUS "llama.cpp: Found")
else()
    message(STATUS "llama.cpp: Not found (LLM backend disabled)")
endif()

# ─────────────────────────────────────────────────────────────────────────────
# Installation
# ─────────────────────────────────────────────────────────────────────────────

install(TARGETS hefaos-ai
    EXPORT HefaosTargets
    LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}
    ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR}
    RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
)

install(
    DIRECTORY include/hefaos
    DESTINATION ${CMAKE_INSTALL_INCLUDEDIR}
)

# ─────────────────────────────────────────────────────────────────────────────
# Tests
# ─────────────────────────────────────────────────────────────────────────────

if(BUILD_TESTING AND TARGET GTest::gtest)
    add_executable(hefaos-ai-tests
        tests/unit/test_model_loader.cpp
        tests/integration/test_perception_pipeline.cpp
    )

    target_link_libraries(hefaos-ai-tests
        PRIVATE
            hefaos-ai
            GTest::gtest
            GTest::gtest_main
            GTest::gmock
    )

    include(GoogleTest)
    gtest_discover_tests(hefaos-ai-tests)
endif()

# ─────────────────────────────────────────────────────────────────────────────
# Benchmarks
# ─────────────────────────────────────────────────────────────────────────────

if(BUILD_BENCHMARKS)
    find_package(benchmark QUIET)
    if(benchmark_FOUND)
        add_executable(hefaos-ai-benchmarks
            benchmarks/bench_onnx_inference.cpp
            benchmarks/bench_tflite_inference.cpp
        )

        target_link_libraries(hefaos-ai-benchmarks
            PRIVATE
                hefaos-ai
                benchmark::benchmark
        )
    endif()
endif()
