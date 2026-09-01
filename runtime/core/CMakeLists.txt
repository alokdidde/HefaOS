# ─────────────────────────────────────────────────────────────────────────────
# Hefaos Core Library
# ─────────────────────────────────────────────────────────────────────────────
#
# Core components:
#   - State Store (ECS-based state management)
#   - Task Graph (DAG scheduler)
#   - Executor (real-time task execution)
#   - IPC (iceoryx zero-copy channels)
#
# ─────────────────────────────────────────────────────────────────────────────

add_library(hefaos-core)
add_library(Hefaos::core ALIAS hefaos-core)

target_sources(hefaos-core
    PRIVATE
        # Source files (to be implemented)
        src/state_store.cpp
        src/task_graph.cpp
        src/executor.cpp
        src/entity.cpp
)

target_include_directories(hefaos-core
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
        $<INSTALL_INTERFACE:include>
)

target_compile_features(hefaos-core PUBLIC cxx_std_20)

# Dependencies
target_link_libraries(hefaos-core
    PUBLIC
        Threads::Threads
)

# Optional dependencies
if(TARGET iceoryx_posh::iceoryx_posh)
    target_link_libraries(hefaos-core PUBLIC iceoryx_posh::iceoryx_posh)
    target_compile_definitions(hefaos-core PUBLIC HEFAOS_HAS_ICEORYX)
endif()

if(TARGET spdlog::spdlog)
    target_link_libraries(hefaos-core PUBLIC spdlog::spdlog)
endif()

if(TARGET fmt::fmt)
    target_link_libraries(hefaos-core PUBLIC fmt::fmt)
endif()

if(TARGET TBB::tbb)
    target_link_libraries(hefaos-core PUBLIC TBB::tbb)
    target_compile_definitions(hefaos-core PUBLIC HEFAOS_HAS_TBB)
endif()

# ─────────────────────────────────────────────────────────────────────────────
# Installation
# ─────────────────────────────────────────────────────────────────────────────

install(TARGETS hefaos-core
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
    add_executable(hefaos-core-tests
        tests/unit/test_entity.cpp
        tests/unit/test_ring_buffer.cpp
        tests/component/test_state_store.cpp
        tests/component/test_task_graph.cpp
    )

    target_link_libraries(hefaos-core-tests
        PRIVATE
            hefaos-core
            GTest::gtest
            GTest::gtest_main
            GTest::gmock
    )

    include(GoogleTest)
    gtest_discover_tests(hefaos-core-tests)
endif()
