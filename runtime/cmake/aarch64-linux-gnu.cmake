# ─────────────────────────────────────────────────────────────────────────────
# aarch64-linux-gnu.cmake - Cross-compilation toolchain for ARM64 Linux
# ─────────────────────────────────────────────────────────────────────────────
#
# Usage:
#   cmake -B build-arm -G Ninja \
#       -DCMAKE_TOOLCHAIN_FILE=cmake/aarch64-linux-gnu.cmake \
#       -DCMAKE_BUILD_TYPE=Release
#
# Prerequisites:
#   sudo apt install gcc-aarch64-linux-gnu g++-aarch64-linux-gnu
#
# ─────────────────────────────────────────────────────────────────────────────

set(CMAKE_SYSTEM_NAME Linux)
set(CMAKE_SYSTEM_PROCESSOR aarch64)

# Cross compilers
set(CMAKE_C_COMPILER aarch64-linux-gnu-gcc)
set(CMAKE_CXX_COMPILER aarch64-linux-gnu-g++)

# Sysroot (optional - set if using custom sysroot)
# set(CMAKE_SYSROOT /path/to/aarch64-sysroot)

# Search paths
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)

# ARM64-specific optimizations
set(CMAKE_C_FLAGS_INIT "-march=armv8-a")
set(CMAKE_CXX_FLAGS_INIT "-march=armv8-a")

# For Raspberry Pi 5 / Orange Pi 5 (Cortex-A76)
# set(CMAKE_C_FLAGS_INIT "-march=armv8.2-a -mcpu=cortex-a76")
# set(CMAKE_CXX_FLAGS_INIT "-march=armv8.2-a -mcpu=cortex-a76")

# For NVIDIA Jetson Orin (Cortex-A78AE)
# set(CMAKE_C_FLAGS_INIT "-march=armv8.2-a -mcpu=cortex-a78ae")
# set(CMAKE_CXX_FLAGS_INIT "-march=armv8.2-a -mcpu=cortex-a78ae")

# QEMU emulation support for testing
set(CMAKE_CROSSCOMPILING_EMULATOR "qemu-aarch64-static;-L;/usr/aarch64-linux-gnu")

# Disable some features that may not work in cross-compilation
set(THREADS_PTHREAD_ARG "0" CACHE STRING "Result from TRY_RUN" FORCE)
