#!/bin/bash
# ─────────────────────────────────────────────────────────────────────────────
# Hefaos Development Environment Setup (Linux/WSL)
# ─────────────────────────────────────────────────────────────────────────────
#
# This script sets up a complete development environment for Hefaos on
# Ubuntu 24.04 or compatible systems (including WSL2).
#
# Usage:
#   chmod +x tools/scripts/setup-dev.sh
#   ./tools/scripts/setup-dev.sh
#
# ─────────────────────────────────────────────────────────────────────────────

set -e

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║       Hefaos Development Environment Setup                ║"
echo "╚═══════════════════════════════════════════════════════════╝"

# ─────────────────────────────────────────────────────────────────────────────
# 1. Update system packages
# ─────────────────────────────────────────────────────────────────────────────

echo -e "\n[1/8] Updating system packages..."
sudo apt-get update && sudo apt-get upgrade -y

# ─────────────────────────────────────────────────────────────────────────────
# 2. Install build tools
# ─────────────────────────────────────────────────────────────────────────────

echo -e "\n[2/8] Installing build tools..."
sudo apt-get install -y \
    build-essential \
    cmake \
    ninja-build \
    clang \
    clang-format \
    clang-tidy \
    lldb \
    gdb \
    git \
    curl \
    wget \
    pkg-config

# ─────────────────────────────────────────────────────────────────────────────
# 3. Install C++ dependencies
# ─────────────────────────────────────────────────────────────────────────────

echo -e "\n[3/8] Installing C++ dependencies..."
sudo apt-get install -y \
    libssl-dev \
    libboost-all-dev \
    libtbb-dev \
    libfmt-dev \
    libspdlog-dev \
    libyaml-cpp-dev \
    libgtest-dev \
    libgmock-dev \
    libarrow-dev \
    libparquet-dev \
    flatbuffers-compiler \
    libflatbuffers-dev \
    lcov

# ─────────────────────────────────────────────────────────────────────────────
# 4. Install cross-compilation toolchain
# ─────────────────────────────────────────────────────────────────────────────

echo -e "\n[4/8] Installing ARM cross-compilation toolchain..."
sudo apt-get install -y \
    gcc-aarch64-linux-gnu \
    g++-aarch64-linux-gnu \
    qemu-user-static \
    qemu-system-arm

# ─────────────────────────────────────────────────────────────────────────────
# 5. Build and install iceoryx
# ─────────────────────────────────────────────────────────────────────────────

echo -e "\n[5/8] Building and installing iceoryx..."
if [ ! -d "/opt/iceoryx" ]; then
    cd /tmp
    git clone https://github.com/eclipse-iceoryx/iceoryx.git --depth 1 --branch v2.0.5
    cd iceoryx
    cmake -B build -G Ninja \
        -DCMAKE_INSTALL_PREFIX=/opt/iceoryx \
        -DCMAKE_BUILD_TYPE=Release \
        -DBUILD_SHARED_LIBS=ON
    cmake --build build --parallel
    sudo cmake --install build
    cd /tmp && rm -rf iceoryx

    # Add to environment
    echo 'export CMAKE_PREFIX_PATH="/opt/iceoryx:$CMAKE_PREFIX_PATH"' >> ~/.bashrc
    echo 'export LD_LIBRARY_PATH="/opt/iceoryx/lib:$LD_LIBRARY_PATH"' >> ~/.bashrc
    echo "  ✓ iceoryx installed to /opt/iceoryx"
else
    echo "  ✓ iceoryx already installed"
fi

# ─────────────────────────────────────────────────────────────────────────────
# 6. Install Node.js and pnpm
# ─────────────────────────────────────────────────────────────────────────────

echo -e "\n[6/8] Installing Node.js and pnpm..."
if ! command -v node &> /dev/null; then
    curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
    sudo apt-get install -y nodejs
fi
sudo npm install -g pnpm@9 turbo
echo "  ✓ Node.js $(node --version) and pnpm installed"

# ─────────────────────────────────────────────────────────────────────────────
# 7. Setup Python environment
# ─────────────────────────────────────────────────────────────────────────────

echo -e "\n[7/8] Setting up Python environment..."
sudo apt-get install -y python3.12 python3.12-venv python3-pip
pip3 install --user \
    mujoco \
    numpy \
    scipy \
    pytest \
    pytest-asyncio \
    black \
    mypy \
    ruff
echo "  ✓ Python environment configured"

# ─────────────────────────────────────────────────────────────────────────────
# 8. Create shell aliases
# ─────────────────────────────────────────────────────────────────────────────

echo -e "\n[8/8] Creating shell aliases..."

cat >> ~/.bashrc << 'EOF'

# ─────────────────────────────────────────────────────────────────────────────
# Hefaos Development Aliases
# ─────────────────────────────────────────────────────────────────────────────

# Build commands
alias hefaos-build="cmake --build build --parallel"
alias hefaos-test="ctest --test-dir build --output-on-failure"
alias hefaos-format="find . -name '*.cpp' -o -name '*.hpp' | xargs clang-format -i"
alias hefaos-lint="cmake --build build --target clang-tidy"

# Cross-compilation helper
hefaos-cross() {
    cmake -B build-arm -G Ninja \
        -DCMAKE_TOOLCHAIN_FILE=cmake/aarch64-linux-gnu.cmake \
        -DCMAKE_BUILD_TYPE=Release
    cmake --build build-arm --parallel
}

# Quick simulation
hefaos-sim() {
    python3 -m hefaos_sim "$@"
}
EOF

echo ""
echo "╔═══════════════════════════════════════════════════════════╗"
echo "║  ✓ Development environment setup complete!                 ║"
echo "╠═══════════════════════════════════════════════════════════╣"
echo "║  Next steps:                                               ║"
echo "║  1. Restart your terminal (or run: source ~/.bashrc)       ║"
echo "║  2. Build C++ runtime:                                     ║"
echo "║     cd runtime && cmake -B build -G Ninja && hefaos-build  ║"
echo "║  3. Build TypeScript SDK:                                  ║"
echo "║     cd sdk && pnpm install && pnpm build                   ║"
echo "╚═══════════════════════════════════════════════════════════╝"
