# ─────────────────────────────────────────────────────────────────────────────
# HefaosConfig.cmake - CMake package configuration for Hefaos
# ─────────────────────────────────────────────────────────────────────────────
#
# This file is used by find_package(Hefaos) to locate the Hefaos libraries.
#
# Exported targets:
#   Hefaos::core    - Core state store, task graph, executor
#   Hefaos::hal     - Hardware abstraction layer
#   Hefaos::ai      - AI runtime (ONNX, TFLite, llama.cpp)
#
# Usage:
#   find_package(Hefaos REQUIRED COMPONENTS core hal ai)
#   target_link_libraries(myapp PRIVATE Hefaos::core Hefaos::hal)
#
# ─────────────────────────────────────────────────────────────────────────────

@PACKAGE_INIT@

include(CMakeFindDependencyMacro)

# Required dependencies
find_dependency(Threads)

# Optional dependencies
find_dependency(iceoryx_posh CONFIG)
find_dependency(iceoryx_hoofs CONFIG)
find_dependency(FlatBuffers)
find_dependency(spdlog)
find_dependency(fmt)

# Include targets
include("${CMAKE_CURRENT_LIST_DIR}/HefaosTargets.cmake" OPTIONAL)

# Component handling
set(_Hefaos_FOUND_COMPONENTS "")

foreach(_comp IN LISTS Hefaos_FIND_COMPONENTS)
    if(TARGET Hefaos::${_comp})
        set(Hefaos_${_comp}_FOUND TRUE)
        list(APPEND _Hefaos_FOUND_COMPONENTS ${_comp})
    else()
        set(Hefaos_${_comp}_FOUND FALSE)
        if(Hefaos_FIND_REQUIRED_${_comp})
            message(FATAL_ERROR "Hefaos component '${_comp}' not found")
        endif()
    endif()
endforeach()

# Compatibility variables
set(HEFAOS_INCLUDE_DIRS "${CMAKE_CURRENT_LIST_DIR}/../../../include")
set(HEFAOS_LIBRARIES Hefaos::core)

check_required_components(Hefaos)

message(STATUS "Found Hefaos ${Hefaos_VERSION}")
message(STATUS "  Components: ${_Hefaos_FOUND_COMPONENTS}")
