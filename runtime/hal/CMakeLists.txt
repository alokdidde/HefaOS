# ─────────────────────────────────────────────────────────────────────────────
# Hefaos HAL (Hardware Abstraction Layer)
# ─────────────────────────────────────────────────────────────────────────────
#
# HAL components:
#   - GPIO interface
#   - Serial/UART communication
#   - CAN bus interface
#   - Camera capture
#   - Backend implementations (Linux, Simulation, Mock)
#
# ─────────────────────────────────────────────────────────────────────────────

add_library(hefaos-hal)
add_library(Hefaos::hal ALIAS hefaos-hal)

target_sources(hefaos-hal
    PRIVATE
        # Source files (to be implemented)
        src/hal.cpp
        src/gpio.cpp
        src/serial.cpp
        src/can.cpp
        src/camera.cpp
        src/device_registry.cpp
        # Linux backend
        src/backends/linux/linux_gpio.cpp
        src/backends/linux/linux_serial.cpp
        # Simulation backend
        src/backends/simulation/sim_gpio.cpp
        src/backends/simulation/sim_serial.cpp
        # Mock backend for testing
        src/backends/mock/mock_hal.cpp
)

target_include_directories(hefaos-hal
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
        $<INSTALL_INTERFACE:include>
)

target_compile_features(hefaos-hal PUBLIC cxx_std_20)

# Dependencies
target_link_libraries(hefaos-hal
    PUBLIC
        Hefaos::core
        Threads::Threads
)

# Optional dependencies
if(TARGET spdlog::spdlog)
    target_link_libraries(hefaos-hal PUBLIC spdlog::spdlog)
endif()

if(TARGET yaml-cpp::yaml-cpp)
    target_link_libraries(hefaos-hal PUBLIC yaml-cpp::yaml-cpp)
elseif(TARGET yaml-cpp)
    target_link_libraries(hefaos-hal PUBLIC yaml-cpp)
endif()

# Platform-specific settings
if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
    target_compile_definitions(hefaos-hal PRIVATE HEFAOS_PLATFORM_LINUX)
endif()

# ─────────────────────────────────────────────────────────────────────────────
# Installation
# ─────────────────────────────────────────────────────────────────────────────

install(TARGETS hefaos-hal
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
    add_executable(hefaos-hal-tests
        tests/unit/test_device_registry.cpp
        tests/integration/test_sensor_fusion.cpp
    )

    target_link_libraries(hefaos-hal-tests
        PRIVATE
            hefaos-hal
            GTest::gtest
            GTest::gtest_main
            GTest::gmock
    )

    include(GoogleTest)
    gtest_discover_tests(hefaos-hal-tests)
endif()
