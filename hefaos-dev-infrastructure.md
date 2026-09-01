# Hefaos Development Infrastructure

## Repository Strategy, Windows Dev Environment & Testing Framework

**Version:** 1.0.0  
**Last Updated:** January 2026

---

## Table of Contents

1. [Repository Structure Decision](#1-repository-structure-decision)
2. [Recommended Architecture: Single Monorepo](#2-recommended-architecture-single-monorepo)
3. [Windows Development Environment](#3-windows-development-environment)
4. [Testing Strategy](#4-testing-strategy)
5. [CI/CD Pipeline](#5-cicd-pipeline)
6. [Quick Start Guide](#6-quick-start-guide)

---

## 1. Repository Structure Decision

### 1.1 Analysis of Hefaos Components

Based on the specifications, Hefaos consists of distinct layers with different characteristics:

| Component | Language | Build System | Release Cycle |
|-----------|----------|--------------|---------------|
| hefaos-core | C++ | CMake/Meson | Slow (stable) |
| hefaos-hal | C++ | CMake | Medium |
| hefaos-ai-runtime | C++ | CMake | Medium |
| hefaos-sdk | TypeScript | npm/esbuild | Fast |
| hefaos-compiler | TypeScript | npm | Fast |
| hefaos-cli | TypeScript | npm | Fast |
| hefaos-vscode | TypeScript | npm | Fast |
| hefaos-simulator | Python/C++ | CMake + pip | Medium |
| hefaos-docs | Markdown | mkdocs | Fast |

### 1.2 Recommendation: Start Simple, Split Later

For a solo developer (or small team), **start with a single monorepo**. The overhead of managing multiple repositories, version matrices, and cross-repo coordination isn't worth it until you have:

- Multiple contributors needing different access levels
- Models/assets exceeding ~500MB slowing down clones
- Community contributions requiring isolation from core
- Different release cycles causing friction

**You can always split later** — it's much easier than merging repos.

### 1.3 Options Comparison

| Approach | Solo Dev | Small Team (2-5) | Larger Team (5+) |
|----------|----------|------------------|------------------|
| **Single Monorepo** | ✅ Recommended | ✅ Good | ⚠️ Consider split |
| **Federated Monorepo** | ❌ Overkill | ⚠️ Maybe | ✅ Recommended |
| **Pure Polyrepo** | ❌ Overkill | ❌ Overhead | ⚠️ Special cases |

---

## 2. Recommended Architecture: Single Monorepo

### 2.1 Why Single Monorepo for Solo Dev

**Pros:**
- One clone, one place — no juggling repos
- Atomic commits — change runtime + SDK + docs in one PR
- Simple CI — one workflow file to start
- Easy refactoring — move code freely across boundaries
- No version matrix headaches

**Cons (acceptable for now):**
- Large clone over time (mitigate with `.gitignore` for models)
- CI runs more than needed (mitigate with path filters)

### 2.2 Repository Structure

```
hefaos/
├── .github/
│   ├── workflows/
│   │   ├── ci.yml                  # Single CI workflow with path filters
│   │   └── release.yml             # Coordinated releases
│   └── CODEOWNERS
│
├── runtime/                         # C++ Components
│   ├── CMakeLists.txt              # Root CMake
│   ├── cmake/                       # CMake modules
│   │   ├── HefaosConfig.cmake
│   │   ├── FindIceoryx.cmake
│   │   └── CrossCompile.cmake
│   │
│   ├── core/                        # hefaos-core library
│   │   ├── CMakeLists.txt
│   │   ├── include/hefaos/
│   │   │   ├── state_store.hpp
│   │   │   ├── task_graph.hpp
│   │   │   ├── executor.hpp
│   │   │   ├── ipc/
│   │   │   │   ├── channel.hpp
│   │   │   │   └── iceoryx_backend.hpp
│   │   │   └── safety/
│   │   │       ├── watchdog.hpp
│   │   │       └── fault_handler.hpp
│   │   ├── src/
│   │   │   ├── state_store.cpp
│   │   │   ├── task_graph.cpp
│   │   │   └── ...
│   │   └── tests/
│   │       ├── unit/
│   │       └── integration/
│   │
│   ├── hal/                         # Hardware abstraction
│   │   ├── CMakeLists.txt
│   │   ├── include/hefaos/hal/
│   │   │   ├── hal.hpp
│   │   │   ├── gpio.hpp
│   │   │   ├── serial.hpp
│   │   │   ├── can.hpp
│   │   │   ├── camera.hpp
│   │   │   └── backends/
│   │   │       ├── linux/
│   │   │       ├── simulation/
│   │   │       └── mock/
│   │   ├── src/
│   │   └── tests/
│   │
│   ├── ai/                          # AI Runtime
│   │   ├── CMakeLists.txt
│   │   ├── include/hefaos/ai/
│   │   │   ├── runtime.hpp
│   │   │   ├── model.hpp
│   │   │   └── backends/
│   │   │       ├── onnx.hpp
│   │   │       ├── tflite.hpp
│   │   │       └── llama.hpp
│   │   ├── src/
│   │   └── tests/
│   │
│   ├── ros2-bridge/                 # Optional ROS2 compatibility
│   │   ├── CMakeLists.txt
│   │   └── ...
│   │
│   └── third_party/                 # Vendored dependencies
│       ├── iceoryx/                 # As git submodule
│       ├── flatbuffers/
│       └── taskflow/
│
├── sdk/                             # TypeScript Components
│   ├── package.json                 # Workspace root
│   ├── pnpm-workspace.yaml
│   ├── turbo.json                   # Turborepo config
│   ├── tsconfig.base.json
│   │
│   ├── packages/
│   │   ├── sdk/                     # @hefaos/sdk
│   │   │   ├── package.json
│   │   │   ├── tsconfig.json
│   │   │   ├── src/
│   │   │   │   ├── index.ts
│   │   │   │   ├── robot.tsx
│   │   │   │   ├── components/
│   │   │   │   ├── tasks/
│   │   │   │   ├── behaviors/
│   │   │   │   └── hooks/
│   │   │   └── tests/
│   │   │
│   │   ├── compiler/                # @hefaos/compiler
│   │   │   ├── package.json
│   │   │   ├── src/
│   │   │   │   ├── parser.ts
│   │   │   │   ├── transformer.ts
│   │   │   │   ├── generators/
│   │   │   │   │   ├── cpp.ts
│   │   │   │   │   ├── flatbuffer.ts
│   │   │   │   │   └── yaml.ts
│   │   │   │   └── validators/
│   │   │   └── tests/
│   │   │
│   │   ├── cli/                     # @hefaos/cli
│   │   │   ├── package.json
│   │   │   ├── src/
│   │   │   │   ├── commands/
│   │   │   │   │   ├── init.ts
│   │   │   │   │   ├── build.ts
│   │   │   │   │   ├── deploy.ts
│   │   │   │   │   ├── simulate.ts
│   │   │   │   │   └── monitor.ts
│   │   │   │   └── index.ts
│   │   │   └── tests/
│   │   │
│   │   ├── vscode/                  # VS Code Extension
│   │   │   ├── package.json
│   │   │   ├── src/
│   │   │   └── tests/
│   │   │
│   │   ├── types/                   # @hefaos/types (shared types)
│   │   │   ├── package.json
│   │   │   └── src/
│   │   │
│   │   └── schemas/                 # @hefaos/schemas (FlatBuffer defs)
│   │       ├── package.json
│   │       └── schemas/
│   │           ├── imu.fbs
│   │           ├── joint.fbs
│   │           └── ...
│   │
│   └── apps/
│       ├── playground/              # Web-based robot playground
│       │   ├── package.json
│       │   └── src/
│       └── dashboard/               # Robot monitoring dashboard
│           ├── package.json
│           └── src/
│
├── simulator/                       # Simulation Integration
│   ├── CMakeLists.txt
│   ├── python/
│   │   ├── hefaos_sim/
│   │   │   ├── __init__.py
│   │   │   ├── mujoco_backend.py
│   │   │   ├── gazebo_backend.py
│   │   │   └── isaac_backend.py
│   │   └── setup.py
│   ├── cpp/                         # Sim HAL backend
│   │   └── sim_hal.cpp
│   └── tests/
│
├── boards/                          # Board Support Packages
│   ├── common/
│   │   ├── kernel-configs/
│   │   └── rt-setup/
│   ├── jetson-orin/
│   │   ├── README.md
│   │   ├── board.yaml
│   │   └── device-tree/
│   ├── raspberry-pi-5/
│   └── orange-pi-5/
│
├── models/                          # Pre-trained Models (gitignored)
│   ├── .gitkeep
│   ├── README.md                    # Instructions for downloading
│   └── download.sh                  # Script to fetch models
│
├── examples/                        # Example Robots
│   ├── simple-arm/
│   │   ├── robot.tsx
│   │   ├── behaviors/
│   │   └── README.md
│   ├── mobile-manipulator/
│   └── templates/
│       ├── basic/
│       └── advanced/
│
├── docs/                            # Documentation
│   ├── mkdocs.yml
│   ├── docs/
│   │   ├── getting-started/
│   │   ├── tutorials/
│   │   ├── api/
│   │   ├── architecture/
│   │   └── migration/
│   └── examples/
│
├── tools/                           # Development tools
│   ├── docker/
│   │   ├── Dockerfile.dev          # Development container
│   │   ├── Dockerfile.ci           # CI container
│   │   └── Dockerfile.release      # Release builds
│   ├── scripts/
│   │   ├── setup-dev.sh
│   │   ├── setup-dev.ps1           # Windows setup
│   │   ├── cross-compile.sh
│   │   └── download-models.sh      # Fetch models from remote
│   └── devcontainer/
│       └── devcontainer.json
│
├── .editorconfig
├── .gitignore
├── .gitmodules                      # For third_party submodules
├── CHANGELOG.md
├── CONTRIBUTING.md
├── LICENSE
└── README.md
```

### 2.3 Handling Large Files (Models)

Don't commit large model files. Use `.gitignore` and a download script:

```gitignore
# .gitignore
models/*.onnx
models/*.gguf
models/*.tflite
models/*.pt
```

```bash
#!/bin/bash
# tools/scripts/download-models.sh

set -e

MODELS_DIR="$(dirname "$0")/../../models"
BASE_URL="https://huggingface.co/hefaos-robotics/models/resolve/main"

mkdir -p "$MODELS_DIR"

echo "Downloading models..."

# Perception models
curl -L "$BASE_URL/yolov8s.onnx" -o "$MODELS_DIR/yolov8s.onnx"
curl -L "$BASE_URL/depth-anything.onnx" -o "$MODELS_DIR/depth-anything.onnx"

# Control models  
curl -L "$BASE_URL/grasp-policy.tflite" -o "$MODELS_DIR/grasp-policy.tflite"

# Optional: Large LLM (only if needed)
if [ "$1" == "--with-llm" ]; then
    curl -L "$BASE_URL/llama3-8b.gguf" -o "$MODELS_DIR/llama3-8b.gguf"
fi

echo "✓ Models downloaded to $MODELS_DIR"
```

### 2.4 Key Design Rules for Future Migration

Maintain **clean directory boundaries** so splitting repos later is trivial:

| Rule | Why |
|------|-----|
| `runtime/` has standalone `CMakeLists.txt` | Can become its own repo |
| `sdk/` has standalone `package.json` | Can become its own repo |
| No imports across `runtime/` ↔ `sdk/` | No circular dependencies |
| `boards/` and `models/` are self-contained | Easy to extract |
| Configuration via files, not hardcoded paths | Repos can live anywhere |

**When to split (signs you've outgrown single repo):**
- Models exceed ~500MB and slow down clones → Extract to `hefaos-models` with Git LFS
- Community wants to contribute examples without core access → Extract to `hefaos-examples`
- Board support needs independent releases → Extract to `hefaos-boards`

**How to split when ready:**

```bash
# Example: Extract models/ to its own repo
cd hefaos
git filter-repo --subdirectory-filter models/
git remote add origin git@github.com:hefaos-robotics/hefaos-models.git
git push -u origin main
```

### 2.5 Version Tracking

```yaml
# hefaos/versions.yaml - Single source of truth
versions:
  hefaos-core: "0.1.0"
  hefaos-hal: "0.1.0"
  hefaos-ai: "0.1.0"
  hefaos-sdk: "0.1.0"
  hefaos-compiler: "0.1.0"
  hefaos-cli: "0.1.0"

compatibility:
  # SDK version ranges compatible with runtime versions
  "sdk@0.1.x": "runtime@0.1.x"
  "sdk@0.2.x": "runtime@0.1.x || runtime@0.2.x"

dependencies:
  iceoryx: "2.0.5"
  flatbuffers: "24.3.25"
  arrow: "15.0.0"
  onnxruntime: "1.17.0"
```

---

## 3. Windows Development Environment

### 3.1 Architecture Overview

Since Hefaos targets Linux ARM boards, Windows development requires a layered approach:

```
┌──────────────────────────────────────────────────────────────────────┐
│                        Windows Host                                   │
├──────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  ┌─────────────────────────────────────────────────────────────────┐ │
│  │  Native Windows Tools                                            │ │
│  │                                                                  │ │
│  │  • VS Code + Hefaos Extension                                    │ │
│  │  • Node.js (TypeScript SDK development)                         │ │
│  │  • Git + GitHub Desktop                                         │ │
│  │  • Docker Desktop                                                │ │
│  └─────────────────────────────────────────────────────────────────┘ │
│                              │                                        │
│                              ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────┐ │
│  │  WSL2 (Ubuntu 24.04)                                            │ │
│  │                                                                  │ │
│  │  • C++ Build Environment (GCC, Clang, CMake)                    │ │
│  │  • Cross-compilation toolchains (aarch64-linux-gnu)             │ │
│  │  • iceoryx, Arrow, FlatBuffers                                  │ │
│  │  • Python + MuJoCo for simulation                               │ │
│  │  • QEMU for ARM emulation testing                               │ │
│  └─────────────────────────────────────────────────────────────────┘ │
│                              │                                        │
│                              ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────┐ │
│  │  Docker Containers (via Docker Desktop + WSL2)                   │ │
│  │                                                                  │ │
│  │  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐       │ │
│  │  │ hefaos-dev     │  │ hefaos-ci      │  │ hefaos-arm     │       │ │
│  │  │               │  │               │  │ (cross-build) │       │ │
│  │  │ Full dev env  │  │ Minimal CI    │  │               │       │ │
│  │  └───────────────┘  └───────────────┘  └───────────────┘       │ │
│  │                                                                  │ │
│  └─────────────────────────────────────────────────────────────────┘ │
│                                                                       │
└──────────────────────────────────────────────────────────────────────┘
```

### 3.2 Setup Script (PowerShell)

```powershell
# tools/scripts/setup-dev.ps1

#Requires -Version 7.0
#Requires -RunAsAdministrator

param(
    [switch]$SkipWSL,
    [switch]$SkipDocker,
    [switch]$SkipNode,
    [switch]$Minimal
)

$ErrorActionPreference = "Stop"

Write-Host "╔═══════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║         Hefaos Development Environment Setup            ║" -ForegroundColor Cyan
Write-Host "╚═══════════════════════════════════════════════════════════╝" -ForegroundColor Cyan

# ──────────────────────────────────────────────────────────────────────
# 1. Check Prerequisites
# ──────────────────────────────────────────────────────────────────────

function Test-Command($Command) {
    return [bool](Get-Command -Name $Command -ErrorAction SilentlyContinue)
}

Write-Host "`n[1/7] Checking prerequisites..." -ForegroundColor Yellow

# Check Windows version
$os = Get-CimInstance -ClassName Win32_OperatingSystem
$build = [int]$os.BuildNumber
if ($build -lt 19041) {
    throw "Windows build 19041+ required (you have $build). Please update Windows."
}
Write-Host "  ✓ Windows build $build" -ForegroundColor Green

# Check virtualization
$hyperv = Get-CimInstance -ClassName Win32_ComputerSystem
if (-not $hyperv.HypervisorPresent) {
    Write-Host "  ⚠ Hyper-V not detected. Enable virtualization in BIOS." -ForegroundColor Yellow
}

# ──────────────────────────────────────────────────────────────────────
# 2. Install Package Managers
# ──────────────────────────────────────────────────────────────────────

Write-Host "`n[2/7] Setting up package managers..." -ForegroundColor Yellow

# Install winget if not present
if (-not (Test-Command winget)) {
    Write-Host "  Installing winget..."
    $url = "https://aka.ms/getwinget"
    $installer = "$env:TEMP\Microsoft.DesktopAppInstaller.msixbundle"
    Invoke-WebRequest -Uri $url -OutFile $installer
    Add-AppxPackage -Path $installer
}
Write-Host "  ✓ winget available" -ForegroundColor Green

# ──────────────────────────────────────────────────────────────────────
# 3. Install Core Tools
# ──────────────────────────────────────────────────────────────────────

Write-Host "`n[3/7] Installing core development tools..." -ForegroundColor Yellow

$packages = @(
    @{id = "Git.Git"; name = "Git"},
    @{id = "Microsoft.VisualStudioCode"; name = "VS Code"},
    @{id = "Microsoft.WindowsTerminal"; name = "Windows Terminal"}
)

if (-not $SkipNode) {
    $packages += @{id = "OpenJS.NodeJS.LTS"; name = "Node.js LTS"}
    $packages += @{id = "pnpm.pnpm"; name = "pnpm"}
}

if (-not $SkipDocker) {
    $packages += @{id = "Docker.DockerDesktop"; name = "Docker Desktop"}
}

foreach ($pkg in $packages) {
    Write-Host "  Installing $($pkg.name)..."
    winget install --id $pkg.id --silent --accept-source-agreements --accept-package-agreements 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Host "    ✓ $($pkg.name) installed" -ForegroundColor Green
    } else {
        Write-Host "    ⚠ $($pkg.name) may already be installed" -ForegroundColor Yellow
    }
}

# ──────────────────────────────────────────────────────────────────────
# 4. Setup WSL2
# ──────────────────────────────────────────────────────────────────────

if (-not $SkipWSL) {
    Write-Host "`n[4/7] Setting up WSL2..." -ForegroundColor Yellow
    
    # Enable WSL feature
    $wslFeature = Get-WindowsOptionalFeature -Online -FeatureName Microsoft-Windows-Subsystem-Linux
    if ($wslFeature.State -ne "Enabled") {
        Write-Host "  Enabling WSL feature (requires restart)..."
        Enable-WindowsOptionalFeature -Online -FeatureName Microsoft-Windows-Subsystem-Linux -NoRestart
        Enable-WindowsOptionalFeature -Online -FeatureName VirtualMachinePlatform -NoRestart
    }
    
    # Set WSL2 as default
    wsl --set-default-version 2 2>$null
    
    # Install Ubuntu 24.04
    $distros = wsl --list --quiet 2>$null
    if ($distros -notcontains "Ubuntu-24.04") {
        Write-Host "  Installing Ubuntu 24.04..."
        wsl --install -d Ubuntu-24.04 --no-launch
        Write-Host "    ✓ Ubuntu 24.04 installed" -ForegroundColor Green
    } else {
        Write-Host "    ✓ Ubuntu 24.04 already installed" -ForegroundColor Green
    }
    
    # Configure WSL memory limits
    $wslConfigPath = "$env:USERPROFILE\.wslconfig"
    $wslConfig = @"
[wsl2]
memory=8GB
processors=4
swap=4GB
localhostForwarding=true

[experimental]
sparseVhd=true
"@
    
    if (-not (Test-Path $wslConfigPath)) {
        Set-Content -Path $wslConfigPath -Value $wslConfig
        Write-Host "    ✓ WSL config created at $wslConfigPath" -ForegroundColor Green
    }
} else {
    Write-Host "`n[4/7] Skipping WSL2 setup" -ForegroundColor Gray
}

# ──────────────────────────────────────────────────────────────────────
# 5. Configure Docker Desktop
# ──────────────────────────────────────────────────────────────────────

if (-not $SkipDocker) {
    Write-Host "`n[5/7] Configuring Docker Desktop..." -ForegroundColor Yellow
    
    $dockerConfigDir = "$env:APPDATA\Docker"
    $dockerSettings = "$dockerConfigDir\settings.json"
    
    if (Test-Path $dockerSettings) {
        $config = Get-Content $dockerSettings | ConvertFrom-Json
        $config.wslEngineEnabled = $true
        $config | ConvertTo-Json -Depth 10 | Set-Content $dockerSettings
        Write-Host "    ✓ Docker configured for WSL2 backend" -ForegroundColor Green
    } else {
        Write-Host "    ⚠ Start Docker Desktop manually after installation" -ForegroundColor Yellow
    }
} else {
    Write-Host "`n[5/7] Skipping Docker configuration" -ForegroundColor Gray
}

# ──────────────────────────────────────────────────────────────────────
# 6. Install VS Code Extensions
# ──────────────────────────────────────────────────────────────────────

Write-Host "`n[6/7] Installing VS Code extensions..." -ForegroundColor Yellow

$extensions = @(
    "ms-vscode-remote.remote-wsl",
    "ms-vscode-remote.remote-containers",
    "ms-vscode.cpptools-extension-pack",
    "dbaeumer.vscode-eslint",
    "esbenp.prettier-vscode",
    "bradlc.vscode-tailwindcss",
    "ms-python.python",
    "usernamehw.errorlens",
    "eamodio.gitlens",
    "GitHub.copilot"
)

if (-not $Minimal) {
    $extensions += @(
        "twxs.cmake",
        "ms-vscode.cmake-tools",
        "llvm-vs-code-extensions.vscode-clangd",
        "vadimcn.vscode-lldb",
        "jnoortheen.xonsh"
    )
}

foreach ($ext in $extensions) {
    code --install-extension $ext --force 2>$null
    Write-Host "    ✓ $ext" -ForegroundColor Green
}

# ──────────────────────────────────────────────────────────────────────
# 7. Create WSL Setup Script
# ──────────────────────────────────────────────────────────────────────

Write-Host "`n[7/7] Creating WSL environment setup script..." -ForegroundColor Yellow

$wslSetupScript = @'
#!/bin/bash
set -e

echo "╔═══════════════════════════════════════════════════════════╗"
echo "║       Hefaos WSL2 Development Environment Setup         ║"
echo "╚═══════════════════════════════════════════════════════════╝"

# Update system
echo -e "\n[1/8] Updating system packages..."
sudo apt update && sudo apt upgrade -y

# Install build essentials
echo -e "\n[2/8] Installing build tools..."
sudo apt install -y \
    build-essential \
    cmake \
    ninja-build \
    clang \
    clang-format \
    clang-tidy \
    lldb \
    gdb \
    pkg-config \
    libssl-dev \
    libboost-all-dev \
    libtbb-dev \
    libfmt-dev \
    libspdlog-dev \
    libyaml-cpp-dev \
    libgtest-dev \
    libgmock-dev \
    lcov

# Install cross-compilation toolchain
echo -e "\n[3/8] Installing ARM cross-compilation toolchain..."
sudo apt install -y \
    gcc-aarch64-linux-gnu \
    g++-aarch64-linux-gnu \
    qemu-user-static \
    qemu-system-arm

# Install iceoryx
echo -e "\n[4/8] Building and installing iceoryx..."
if [ ! -d "/opt/iceoryx" ]; then
    cd /tmp
    git clone https://github.com/eclipse-iceoryx/iceoryx.git --depth 1 --branch v2.0.5
    cd iceoryx
    cmake -B build -G Ninja \
        -DCMAKE_INSTALL_PREFIX=/opt/iceoryx \
        -DBUILD_SHARED_LIBS=ON
    cmake --build build --parallel
    sudo cmake --install build
    echo 'export CMAKE_PREFIX_PATH="/opt/iceoryx:$CMAKE_PREFIX_PATH"' >> ~/.bashrc
fi
echo "  ✓ iceoryx installed"

# Install Apache Arrow
echo -e "\n[5/8] Installing Apache Arrow..."
sudo apt install -y \
    libarrow-dev \
    libparquet-dev \
    libarrow-dataset-dev

# Install FlatBuffers
echo -e "\n[6/8] Installing FlatBuffers..."
sudo apt install -y flatbuffers-compiler libflatbuffers-dev

# Install Python environment
echo -e "\n[7/8] Setting up Python environment..."
sudo apt install -y python3.12 python3.12-venv python3-pip
python3 -m pip install --user pipx
pipx ensurepath
pipx install poetry
pipx install mujoco
pipx install uv

# Setup development directories
echo -e "\n[8/8] Creating development directories..."
mkdir -p ~/hefaos-dev
mkdir -p ~/.local/share/hefaos

# Create shell aliases
cat >> ~/.bashrc << 'EOF'

# Hefaos Development Aliases
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
    cd ~/hefaos-dev/hefaos/simulator
    python3 -m hefaos_sim "$@"
}
EOF

echo ""
echo "╔═══════════════════════════════════════════════════════════╗"
echo "║  ✓ WSL2 environment setup complete!                       ║"
echo "╠═══════════════════════════════════════════════════════════╣"
echo "║  Next steps:                                              ║"
echo "║  1. Restart your terminal                                 ║"
echo "║  2. Clone the Hefaos repo:                                 ║"
echo "║     git clone https://github.com/hefaos-robotics/hefaos     ║"
echo "║  3. Open in VS Code:                                      ║"
echo "║     code ~/hefaos-dev/hefaos                                ║"
echo "╚═══════════════════════════════════════════════════════════╝"
'@

$scriptPath = "$env:USERPROFILE\setup-hefaos-wsl.sh"
Set-Content -Path $scriptPath -Value $wslSetupScript -Encoding utf8NoBOM
Write-Host "    ✓ WSL setup script created: $scriptPath" -ForegroundColor Green

# ──────────────────────────────────────────────────────────────────────
# Summary
# ──────────────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "╔═══════════════════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║  ✓ Windows setup complete!                                ║" -ForegroundColor Green
Write-Host "╠═══════════════════════════════════════════════════════════╣" -ForegroundColor Green
Write-Host "║  Next steps:                                              ║" -ForegroundColor Green
Write-Host "║                                                           ║" -ForegroundColor Green
Write-Host "║  1. Restart your computer (required for WSL2)             ║" -ForegroundColor Green
Write-Host "║                                                           ║" -ForegroundColor Green
Write-Host "║  2. Open Ubuntu from Start Menu and create user           ║" -ForegroundColor Green
Write-Host "║                                                           ║" -ForegroundColor Green
Write-Host "║  3. Run WSL setup script:                                 ║" -ForegroundColor Green
Write-Host "║     wsl bash /mnt/c/Users/$env:USERNAME/setup-hefaos-wsl.sh║" -ForegroundColor Cyan
Write-Host "║                                                           ║" -ForegroundColor Green
Write-Host "║  4. Start Docker Desktop                                  ║" -ForegroundColor Green
Write-Host "║                                                           ║" -ForegroundColor Green
Write-Host "╚═══════════════════════════════════════════════════════════╝" -ForegroundColor Green
```

### 3.3 Development Container (DevContainer)

```json
// tools/devcontainer/devcontainer.json
{
  "name": "Hefaos Dev",
  "build": {
    "dockerfile": "../docker/Dockerfile.dev",
    "context": "../..",
    "args": {
      "VARIANT": "noble"
    }
  },
  
  "features": {
    "ghcr.io/devcontainers/features/common-utils:2": {
      "installZsh": true,
      "configureZshAsDefaultShell": true,
      "installOhMyZsh": true
    },
    "ghcr.io/devcontainers/features/node:1": {
      "version": "20"
    },
    "ghcr.io/devcontainers/features/python:1": {
      "version": "3.12"
    }
  },

  "customizations": {
    "vscode": {
      "extensions": [
        "ms-vscode.cpptools-extension-pack",
        "twxs.cmake",
        "ms-vscode.cmake-tools",
        "llvm-vs-code-extensions.vscode-clangd",
        "vadimcn.vscode-lldb",
        "dbaeumer.vscode-eslint",
        "esbenp.prettier-vscode",
        "ms-python.python",
        "usernamehw.errorlens"
      ],
      "settings": {
        "cmake.configureOnOpen": true,
        "cmake.generator": "Ninja",
        "C_Cpp.default.configurationProvider": "ms-vscode.cmake-tools",
        "editor.formatOnSave": true,
        "[cpp]": {
          "editor.defaultFormatter": "llvm-vs-code-extensions.vscode-clangd"
        },
        "[typescript]": {
          "editor.defaultFormatter": "esbenp.prettier-vscode"
        }
      }
    }
  },

  "mounts": [
    "source=${localWorkspaceFolder},target=/workspace,type=bind,consistency=cached",
    "source=hefaos-build-cache,target=/workspace/build,type=volume",
    "source=hefaos-pnpm-store,target=/root/.local/share/pnpm,type=volume"
  ],

  "runArgs": [
    "--cap-add=SYS_PTRACE",
    "--security-opt", "seccomp=unconfined",
    "--privileged"
  ],

  "postCreateCommand": "bash -c 'cd /workspace && cmake -B build -G Ninja && cd sdk && pnpm install'",
  
  "remoteUser": "root",
  "workspaceFolder": "/workspace"
}
```

### 3.4 Docker Development Image

```dockerfile
# tools/docker/Dockerfile.dev

FROM ubuntu:24.04 AS base

ARG DEBIAN_FRONTEND=noninteractive

# System packages
RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    ninja-build \
    clang-18 \
    clang-format-18 \
    clang-tidy-18 \
    lldb-18 \
    gdb \
    git \
    curl \
    wget \
    pkg-config \
    # Dependencies
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
    # Cross-compilation
    gcc-aarch64-linux-gnu \
    g++-aarch64-linux-gnu \
    qemu-user-static \
    # Python
    python3.12 \
    python3.12-venv \
    python3-pip \
    && rm -rf /var/lib/apt/lists/*

# Set clang as default
RUN update-alternatives --install /usr/bin/clang clang /usr/bin/clang-18 100 && \
    update-alternatives --install /usr/bin/clang++ clang++ /usr/bin/clang++-18 100 && \
    update-alternatives --install /usr/bin/clang-format clang-format /usr/bin/clang-format-18 100 && \
    update-alternatives --install /usr/bin/clang-tidy clang-tidy /usr/bin/clang-tidy-18 100

# Build iceoryx
FROM base AS iceoryx-builder
WORKDIR /tmp
RUN git clone https://github.com/eclipse-iceoryx/iceoryx.git --depth 1 --branch v2.0.5 && \
    cd iceoryx && \
    cmake -B build -G Ninja \
        -DCMAKE_INSTALL_PREFIX=/opt/iceoryx \
        -DCMAKE_BUILD_TYPE=Release \
        -DBUILD_SHARED_LIBS=ON && \
    cmake --build build --parallel && \
    cmake --install build

# Final development image
FROM base AS dev

# Copy iceoryx
COPY --from=iceoryx-builder /opt/iceoryx /opt/iceoryx
ENV CMAKE_PREFIX_PATH="/opt/iceoryx:${CMAKE_PREFIX_PATH}"
ENV LD_LIBRARY_PATH="/opt/iceoryx/lib:${LD_LIBRARY_PATH}"

# Node.js (for SDK development)
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - && \
    apt-get install -y nodejs && \
    npm install -g pnpm@9 turbo

# Python packages
RUN pip3 install --break-system-packages \
    mujoco \
    numpy \
    scipy \
    pytest \
    pytest-asyncio \
    black \
    mypy

# Workspace setup
WORKDIR /workspace
ENV HEFAOS_DEV=1

# Volume for build artifacts
VOLUME ["/workspace/build", "/workspace/sdk/node_modules"]

CMD ["/bin/bash"]
```

### 3.5 VS Code Workspace Configuration

```jsonc
// hefaos.code-workspace
{
  "folders": [
    {
      "name": "🦖 Hefaos Root",
      "path": "."
    },
    {
      "name": "⚙️ Runtime (C++)",
      "path": "runtime"
    },
    {
      "name": "📦 SDK (TypeScript)",
      "path": "sdk"
    },
    {
      "name": "🎮 Simulator",
      "path": "simulator"
    },
    {
      "name": "📚 Documentation",
      "path": "docs"
    }
  ],
  
  "settings": {
    // C++ Settings
    "cmake.sourceDirectory": "${workspaceFolder:⚙️ Runtime (C++)}", 
    "cmake.buildDirectory": "${workspaceFolder:⚙️ Runtime (C++)}/build",
    "cmake.generator": "Ninja",
    "cmake.configureSettings": {
      "CMAKE_EXPORT_COMPILE_COMMANDS": "ON",
      "BUILD_TESTING": "ON"
    },
    "clangd.arguments": [
      "--compile-commands-dir=${workspaceFolder:⚙️ Runtime (C++)}/build",
      "--header-insertion=iwyu",
      "--clang-tidy"
    ],
    
    // TypeScript Settings
    "typescript.tsdk": "sdk/node_modules/typescript/lib",
    "eslint.workingDirectories": [
      { "directory": "sdk", "changeProcessCWD": true }
    ],
    
    // Editor Settings
    "editor.formatOnSave": true,
    "editor.codeActionsOnSave": {
      "source.fixAll.eslint": "explicit",
      "source.organizeImports": "explicit"
    },
    
    // File Associations
    "files.associations": {
      "*.fbs": "flatbuffers",
      "*.tsx": "typescriptreact"
    },
    
    // Search Exclusions
    "search.exclude": {
      "**/node_modules": true,
      "**/build": true,
      "**/dist": true,
      "**/.pnpm-store": true
    },
    
    // Task Runner
    "task.autoDetect": "off"
  },
  
  "tasks": {
    "version": "2.0.0",
    "tasks": [
      {
        "label": "🔨 Build Runtime (Debug)",
        "type": "shell",
        "command": "cmake --build build --parallel",
        "options": {
          "cwd": "${workspaceFolder:⚙️ Runtime (C++)}"
        },
        "group": {
          "kind": "build",
          "isDefault": true
        },
        "problemMatcher": "$gcc"
      },
      {
        "label": "🧪 Test Runtime",
        "type": "shell",
        "command": "ctest --test-dir build --output-on-failure",
        "options": {
          "cwd": "${workspaceFolder:⚙️ Runtime (C++)}"
        },
        "group": "test",
        "problemMatcher": "$gcc"
      },
      {
        "label": "📦 Build SDK",
        "type": "shell",
        "command": "pnpm build",
        "options": {
          "cwd": "${workspaceFolder:📦 SDK (TypeScript)}"
        },
        "group": "build",
        "problemMatcher": "$tsc"
      },
      {
        "label": "🧪 Test SDK",
        "type": "shell",
        "command": "pnpm test",
        "options": {
          "cwd": "${workspaceFolder:📦 SDK (TypeScript)}"
        },
        "group": "test"
      },
      {
        "label": "🎯 Cross-compile for ARM",
        "type": "shell",
        "command": "cmake -B build-arm -G Ninja -DCMAKE_TOOLCHAIN_FILE=cmake/aarch64-linux-gnu.cmake && cmake --build build-arm --parallel",
        "options": {
          "cwd": "${workspaceFolder:⚙️ Runtime (C++)}"
        },
        "group": "build"
      }
    ]
  },
  
  "launch": {
    "version": "0.2.0",
    "configurations": [
      {
        "name": "🔬 Debug Unit Test",
        "type": "lldb",
        "request": "launch",
        "program": "${workspaceFolder:⚙️ Runtime (C++)}/build/core/tests/test_state_store",
        "args": [],
        "cwd": "${workspaceFolder:⚙️ Runtime (C++)}"
      },
      {
        "name": "🤖 Debug Simulation",
        "type": "python",
        "request": "launch",
        "program": "${workspaceFolder:🎮 Simulator}/python/hefaos_sim/__main__.py",
        "args": ["--robot", "examples/simple-arm"],
        "cwd": "${workspaceFolder:🎮 Simulator}"
      },
      {
        "name": "📝 Debug Compiler",
        "type": "node",
        "request": "launch",
        "program": "${workspaceFolder:📦 SDK (TypeScript)}/packages/compiler/src/index.ts",
        "args": ["compile", "examples/simple-arm/robot.tsx"],
        "cwd": "${workspaceFolder:📦 SDK (TypeScript)}",
        "runtimeArgs": ["--loader", "tsx"]
      }
    ]
  },
  
  "extensions": {
    "recommendations": [
      "ms-vscode-remote.remote-wsl",
      "ms-vscode-remote.remote-containers",
      "ms-vscode.cpptools-extension-pack",
      "llvm-vs-code-extensions.vscode-clangd",
      "twxs.cmake",
      "ms-vscode.cmake-tools",
      "dbaeumer.vscode-eslint",
      "esbenp.prettier-vscode",
      "ms-python.python",
      "vadimcn.vscode-lldb",
      "usernamehw.errorlens",
      "eamodio.gitlens"
    ]
  }
}
```

---

## 4. Testing Strategy

### 4.1 Testing Pyramid Overview

```
                    ┌───────────────────┐
                    │   System Tests    │  ← Real robot / HIL
                    │   (E2E on HW)     │     
                   ─┴───────────────────┴─
                  ┌─────────────────────────┐
                  │   Integration Tests     │  ← Multi-component
                  │   (Simulation)          │     MuJoCo / Gazebo
                 ─┴─────────────────────────┴─
                ┌───────────────────────────────┐
                │      Component Tests          │  ← Single process
                │   (State Store, Scheduler)    │     Mocked dependencies
               ─┴───────────────────────────────┴─
              ┌─────────────────────────────────────┐
              │          Unit Tests                 │  ← Pure functions
              │   (Data structures, algorithms)    │     No I/O
             ─┴─────────────────────────────────────┴─
            ┌───────────────────────────────────────────┐
            │           Static Analysis                 │  ← Compile-time
            │   (Type checking, linting, MISRA)        │     
            └───────────────────────────────────────────┘
```

### 4.2 Test Categories & Organization

```
runtime/
├── core/
│   └── tests/
│       ├── unit/                    # Pure function tests
│       │   ├── test_ring_buffer.cpp
│       │   ├── test_entity_pool.cpp
│       │   └── test_dag_scheduler.cpp
│       ├── component/               # Component-level tests
│       │   ├── test_state_store.cpp
│       │   ├── test_task_graph.cpp
│       │   └── test_executor.cpp
│       └── integration/             # Multi-component tests
│           ├── test_ipc_roundtrip.cpp
│           └── test_full_pipeline.cpp
│
├── hal/
│   └── tests/
│       ├── unit/
│       │   └── test_device_registry.cpp
│       ├── mock/                    # Mock HAL backends
│       │   ├── mock_gpio.hpp
│       │   ├── mock_serial.hpp
│       │   └── mock_can.hpp
│       └── integration/
│           └── test_sensor_fusion.cpp
│
├── ai/
│   └── tests/
│       ├── unit/
│       │   └── test_model_loader.cpp
│       ├── benchmark/               # Performance benchmarks
│       │   ├── bench_onnx_inference.cpp
│       │   └── bench_tflite_inference.cpp
│       └── integration/
│           └── test_perception_pipeline.cpp
│
sdk/
└── packages/
    ├── sdk/
    │   └── tests/
    │       ├── unit/
    │       │   ├── robot.test.ts
    │       │   └── components.test.ts
    │       └── integration/
    │           └── compiler-output.test.ts
    │
    ├── compiler/
    │   └── tests/
    │       ├── unit/
    │       │   ├── parser.test.ts
    │       │   ├── transformer.test.ts
    │       │   └── generators/
    │       │       ├── cpp.test.ts
    │       │       └── flatbuffer.test.ts
    │       ├── snapshots/           # Golden file tests
    │       │   ├── simple-arm/
    │       │   │   ├── input.tsx
    │       │   │   ├── expected.hpp
    │       │   │   └── expected.yaml
    │       │   └── mobile-base/
    │       └── e2e/
    │           └── full-compilation.test.ts
    │
    └── cli/
        └── tests/
            ├── unit/
            └── e2e/
                ├── init.test.ts
                ├── build.test.ts
                └── deploy.test.ts

simulator/
└── tests/
    ├── unit/
    │   └── test_physics_utils.py
    ├── scenarios/                   # Scenario-based tests
    │   ├── test_pick_and_place.py
    │   ├── test_locomotion.py
    │   └── test_collision_avoidance.py
    └── regression/                  # Behavior regression tests
        └── test_trajectory_tracking.py
```

### 4.3 Unit Testing (C++ Runtime)

```cpp
// runtime/core/tests/unit/test_ring_buffer.cpp

#include <gtest/gtest.h>
#include <hefaos/core/ring_buffer.hpp>

namespace hefaos::core::test {

class RingBufferTest : public ::testing::Test {
protected:
    void SetUp() override {
        buffer_ = std::make_unique<RingBuffer<int, 8>>();
    }
    
    std::unique_ptr<RingBuffer<int, 8>> buffer_;
};

TEST_F(RingBufferTest, StartsEmpty) {
    EXPECT_TRUE(buffer_->empty());
    EXPECT_EQ(buffer_->size(), 0);
}

TEST_F(RingBufferTest, PushIncreasesSize) {
    buffer_->push(42);
    EXPECT_FALSE(buffer_->empty());
    EXPECT_EQ(buffer_->size(), 1);
}

TEST_F(RingBufferTest, PopReturnsInFIFOOrder) {
    buffer_->push(1);
    buffer_->push(2);
    buffer_->push(3);
    
    EXPECT_EQ(buffer_->pop(), 1);
    EXPECT_EQ(buffer_->pop(), 2);
    EXPECT_EQ(buffer_->pop(), 3);
}

TEST_F(RingBufferTest, OverwritesOldestWhenFull) {
    for (int i = 0; i < 10; ++i) {  // Buffer capacity is 8
        buffer_->push(i);
    }
    
    EXPECT_EQ(buffer_->size(), 8);
    EXPECT_EQ(buffer_->pop(), 2);  // 0 and 1 were overwritten
}

// Parameterized test for different sizes
class RingBufferSizeTest : public ::testing::TestWithParam<size_t> {};

TEST_P(RingBufferSizeTest, MaintainsCapacity) {
    const size_t capacity = GetParam();
    RingBuffer<int, 16> buffer;  // Max capacity
    
    for (size_t i = 0; i < capacity * 2; ++i) {
        buffer.push(static_cast<int>(i));
    }
    
    EXPECT_LE(buffer.size(), 16);
}

INSTANTIATE_TEST_SUITE_P(
    CapacityTests,
    RingBufferSizeTest,
    ::testing::Values(4, 8, 16, 32)
);

}  // namespace hefaos::core::test
```

### 4.4 Component Testing (State Store)

```cpp
// runtime/core/tests/component/test_state_store.cpp

#include <gtest/gtest.h>
#include <gmock/gmock.h>
#include <hefaos/state_store.hpp>
#include <hefaos/components/imu.hpp>
#include <hefaos/components/joint.hpp>

namespace hefaos::test {

using ::testing::_;
using ::testing::Return;
using ::testing::ElementsAre;

class StateStoreTest : public ::testing::Test {
protected:
    void SetUp() override {
        store_ = std::make_unique<StateStore>(StateStoreConfig{
            .max_entities = 1000,
            .enable_versioning = true
        });
    }
    
    std::unique_ptr<StateStore> store_;
};

// ─────────────────────────────────────────────────────────────────────
// Entity Lifecycle Tests
// ─────────────────────────────────────────────────────────────────────

TEST_F(StateStoreTest, CreateEntity_ReturnsUniqueIds) {
    auto id1 = store_->create_entity();
    auto id2 = store_->create_entity();
    auto id3 = store_->create_entity();
    
    EXPECT_NE(id1, id2);
    EXPECT_NE(id2, id3);
    EXPECT_NE(id1, id3);
}

TEST_F(StateStoreTest, DestroyEntity_MarksDead) {
    auto id = store_->create_entity();
    EXPECT_TRUE(store_->is_alive(id));
    
    store_->destroy_entity(id);
    EXPECT_FALSE(store_->is_alive(id));
}

TEST_F(StateStoreTest, EntityIdReuse_HasDifferentGeneration) {
    auto id1 = store_->create_entity();
    store_->destroy_entity(id1);
    auto id2 = store_->create_entity();
    
    // Same index but different generation
    EXPECT_EQ(id1.index(), id2.index());
    EXPECT_NE(id1.generation(), id2.generation());
}

// ─────────────────────────────────────────────────────────────────────
// Component CRUD Tests
// ─────────────────────────────────────────────────────────────────────

TEST_F(StateStoreTest, SetAndGetComponent) {
    auto entity = store_->create_entity();
    
    IMUReading reading{
        .linear_acceleration = {0.0f, 0.0f, 9.81f},
        .angular_velocity = {0.0f, 0.0f, 0.0f},
        .timestamp_ns = 1234567890
    };
    
    store_->set<IMUReading>(entity, reading);
    
    auto* retrieved = store_->get<IMUReading>(entity);
    ASSERT_NE(retrieved, nullptr);
    EXPECT_FLOAT_EQ(retrieved->linear_acceleration.z, 9.81f);
    EXPECT_EQ(retrieved->timestamp_ns, 1234567890);
}

TEST_F(StateStoreTest, GetNonexistentComponent_ReturnsNull) {
    auto entity = store_->create_entity();
    // Don't set any component
    
    auto* result = store_->get<IMUReading>(entity);
    EXPECT_EQ(result, nullptr);
}

TEST_F(StateStoreTest, MultipleComponentTypes) {
    auto entity = store_->create_entity();
    
    store_->set<IMUReading>(entity, IMUReading{});
    store_->set<JointState>(entity, JointState{.positions = {1.0, 2.0, 3.0}});
    
    EXPECT_NE(store_->get<IMUReading>(entity), nullptr);
    EXPECT_NE(store_->get<JointState>(entity), nullptr);
    
    auto* joints = store_->get<JointState>(entity);
    EXPECT_THAT(joints->positions, ElementsAre(1.0, 2.0, 3.0));
}

// ─────────────────────────────────────────────────────────────────────
// Query Tests
// ─────────────────────────────────────────────────────────────────────

TEST_F(StateStoreTest, QueryByComponents) {
    // Create entities with different component combinations
    auto robot1 = store_->create_entity();
    store_->set<IMUReading>(robot1, IMUReading{});
    store_->set<JointState>(robot1, JointState{});
    
    auto robot2 = store_->create_entity();
    store_->set<IMUReading>(robot2, IMUReading{});
    // No JointState
    
    auto sensor = store_->create_entity();
    store_->set<IMUReading>(sensor, IMUReading{});
    
    // Query for entities with both IMU and Joints
    auto results = store_->query<IMUReading, JointState>();
    EXPECT_EQ(results.size(), 1);
    EXPECT_EQ(results[0], robot1);
}

TEST_F(StateStoreTest, QueryWithCallback) {
    for (int i = 0; i < 100; ++i) {
        auto e = store_->create_entity();
        store_->set<IMUReading>(e, IMUReading{
            .linear_acceleration = {0, 0, static_cast<float>(i)}
        });
    }
    
    float sum = 0.0f;
    store_->for_each<IMUReading>([&sum](EntityId, const IMUReading& imu) {
        sum += imu.linear_acceleration.z;
    });
    
    EXPECT_FLOAT_EQ(sum, 4950.0f);  // Sum of 0..99
}

// ─────────────────────────────────────────────────────────────────────
// Version/History Tests
// ─────────────────────────────────────────────────────────────────────

TEST_F(StateStoreTest, Versioning_TracksHistory) {
    auto entity = store_->create_entity();
    
    for (int i = 0; i < 5; ++i) {
        store_->set<IMUReading>(entity, IMUReading{
            .linear_acceleration = {0, 0, static_cast<float>(i)},
            .timestamp_ns = static_cast<uint64_t>(i * 1000000)
        });
    }
    
    // Get historical value
    auto* v2 = store_->get_at_version<IMUReading>(entity, 2);
    ASSERT_NE(v2, nullptr);
    EXPECT_FLOAT_EQ(v2->linear_acceleration.z, 2.0f);
}

// ─────────────────────────────────────────────────────────────────────
// Concurrency Tests (Require Threading)
// ─────────────────────────────────────────────────────────────────────

TEST_F(StateStoreTest, ConcurrentReads_NoDataRace) {
    auto entity = store_->create_entity();
    store_->set<IMUReading>(entity, IMUReading{
        .linear_acceleration = {1.0f, 2.0f, 3.0f}
    });
    
    std::vector<std::thread> readers;
    std::atomic<int> success_count{0};
    
    for (int i = 0; i < 10; ++i) {
        readers.emplace_back([this, entity, &success_count]() {
            for (int j = 0; j < 1000; ++j) {
                auto* imu = store_->get<IMUReading>(entity);
                if (imu && imu->linear_acceleration.x == 1.0f) {
                    success_count++;
                }
            }
        });
    }
    
    for (auto& t : readers) {
        t.join();
    }
    
    EXPECT_EQ(success_count, 10000);
}

}  // namespace hefaos::test
```

### 4.5 Timing & Real-Time Tests

```cpp
// runtime/core/tests/component/test_timing.cpp

#include <gtest/gtest.h>
#include <hefaos/executor.hpp>
#include <hefaos/task_graph.hpp>
#include <chrono>
#include <thread>
#include <numeric>

namespace hefaos::test {

using namespace std::chrono;
using namespace std::chrono_literals;

class TimingTest : public ::testing::Test {
protected:
    void SetUp() override {
        config_.rt_cores = {2, 3};  // Use isolated cores if available
        config_.enable_rt_scheduling = CanUseRTScheduling();
        executor_ = std::make_unique<Executor>(config_);
    }
    
    bool CanUseRTScheduling() {
        // Check if we have CAP_SYS_NICE or running as root
        return geteuid() == 0;
    }
    
    ExecutorConfig config_;
    std::unique_ptr<Executor> executor_;
};

TEST_F(TimingTest, PeriodicTask_MeetsDeadlines) {
    constexpr auto period = 1ms;
    constexpr auto max_jitter = 100us;  // Acceptable jitter
    constexpr int iterations = 100;
    
    std::vector<nanoseconds> actual_periods;
    actual_periods.reserve(iterations);
    
    auto last_time = steady_clock::now();
    int count = 0;
    
    TaskGraph graph;
    graph.add_task({
        .name = "timing_test",
        .execute = [&]() {
            auto now = steady_clock::now();
            if (count > 0) {  // Skip first iteration
                actual_periods.push_back(now - last_time);
            }
            last_time = now;
            count++;
        },
        .timing = {
            .period = period,
            .deadline = 800us,
            .priority = TaskPriority::RealTime
        }
    });
    
    executor_->set_graph(graph);
    executor_->run_for(milliseconds(iterations) + 10ms);
    
    // Analyze timing
    auto sum = std::accumulate(actual_periods.begin(), actual_periods.end(), 
                               nanoseconds(0));
    auto avg = sum / actual_periods.size();
    
    auto min_period = *std::min_element(actual_periods.begin(), actual_periods.end());
    auto max_period = *std::max_element(actual_periods.begin(), actual_periods.end());
    
    // Report statistics
    std::cout << "Timing Statistics:\n"
              << "  Average period: " << duration_cast<microseconds>(avg).count() << " μs\n"
              << "  Min period: " << duration_cast<microseconds>(min_period).count() << " μs\n"
              << "  Max period: " << duration_cast<microseconds>(max_period).count() << " μs\n"
              << "  Jitter: " << duration_cast<microseconds>(max_period - min_period).count() << " μs\n";
    
    // Assertions
    EXPECT_NEAR(avg.count(), period.count() * 1000000, max_jitter.count() * 1000);
    EXPECT_LT(max_period - min_period, max_jitter * 2);
}

TEST_F(TimingTest, DeadlineMiss_TriggersCallback) {
    bool deadline_missed = false;
    
    TaskGraph graph;
    graph.add_task({
        .name = "slow_task",
        .execute = []() {
            std::this_thread::sleep_for(5ms);  // Deliberately miss deadline
        },
        .timing = {
            .period = 10ms,
            .deadline = 1ms,
            .priority = TaskPriority::Normal
        },
        .on_deadline_miss = [&deadline_missed](const TaskInfo&) {
            deadline_missed = true;
        }
    });
    
    executor_->set_graph(graph);
    executor_->run_for(50ms);
    
    EXPECT_TRUE(deadline_missed);
}

TEST_F(TimingTest, PriorityInversion_Handled) {
    // Test that high-priority tasks preempt lower priority ones
    std::vector<std::string> execution_order;
    std::mutex order_mutex;
    
    TaskGraph graph;
    
    // Low priority task that holds a resource
    graph.add_task({
        .name = "low_priority",
        .execute = [&]() {
            std::lock_guard<std::mutex> lock(order_mutex);
            std::this_thread::sleep_for(2ms);
            execution_order.push_back("low");
        },
        .timing = { .priority = TaskPriority::Low }
    });
    
    // High priority task
    graph.add_task({
        .name = "high_priority",
        .execute = [&]() {
            std::lock_guard<std::mutex> lock(order_mutex);
            execution_order.push_back("high");
        },
        .timing = { .priority = TaskPriority::RealTime }
    });
    
    executor_->set_graph(graph);
    executor_->run_once();
    
    // High priority should execute first (or be prioritized)
    if (config_.enable_rt_scheduling) {
        EXPECT_EQ(execution_order.front(), "high");
    }
}

// Benchmark-style test for latency measurement
TEST_F(TimingTest, DISABLED_Benchmark_ControlLoopLatency) {
    // This test is disabled by default as it requires RT privileges
    // Run with: --gtest_also_run_disabled_tests
    
    constexpr int iterations = 10000;
    std::vector<nanoseconds> latencies;
    latencies.reserve(iterations);
    
    TaskGraph graph;
    graph.add_task({
        .name = "control_loop",
        .execute = [&]() {
            auto start = steady_clock::now();
            
            // Simulate control computation
            volatile float x = 0;
            for (int i = 0; i < 1000; ++i) {
                x += std::sin(static_cast<float>(i));
            }
            
            auto end = steady_clock::now();
            latencies.push_back(end - start);
        },
        .timing = {
            .period = 1ms,
            .deadline = 800us,
            .priority = TaskPriority::RealTime
        }
    });
    
    executor_->set_graph(graph);
    executor_->run_for(seconds(iterations / 1000 + 1));
    
    // Calculate percentiles
    std::sort(latencies.begin(), latencies.end());
    auto p50 = latencies[latencies.size() * 50 / 100];
    auto p95 = latencies[latencies.size() * 95 / 100];
    auto p99 = latencies[latencies.size() * 99 / 100];
    auto max_lat = latencies.back();
    
    std::cout << "Control Loop Latency:\n"
              << "  P50: " << duration_cast<microseconds>(p50).count() << " μs\n"
              << "  P95: " << duration_cast<microseconds>(p95).count() << " μs\n"
              << "  P99: " << duration_cast<microseconds>(p99).count() << " μs\n"
              << "  Max: " << duration_cast<microseconds>(max_lat).count() << " μs\n";
    
    // For hard RT, P99 should be under deadline
    EXPECT_LT(p99, 800us);
}

}  // namespace hefaos::test
```

### 4.6 SDK Compiler Tests (TypeScript)

```typescript
// sdk/packages/compiler/tests/unit/generators/cpp.test.ts

import { describe, it, expect, beforeEach } from 'vitest';
import { CppGenerator } from '../../../src/generators/cpp';
import { parseRobotDefinition } from '../../../src/parser';

describe('CppGenerator', () => {
  let generator: CppGenerator;

  beforeEach(() => {
    generator = new CppGenerator({
      outputDir: '/tmp/test-output',
      namespace: 'hefaos::generated'
    });
  });

  describe('Component Generation', () => {
    it('generates correct header for IMU component', () => {
      const input = `
        import { defineComponent, Schema } from '@hefaos/sdk';
        
        export const IMUSensor = defineComponent({
          name: 'IMUSensor',
          props: {
            port: Schema.string(),
            rate: Schema.int32(),
          },
          state: {
            linearAcceleration: Schema.vec3(),
            angularVelocity: Schema.vec3(),
            timestamp: Schema.uint64(),
          }
        });
      `;

      const ast = parseRobotDefinition(input);
      const output = generator.generateComponent(ast.components[0]);

      expect(output).toContain('#pragma once');
      expect(output).toContain('namespace hefaos::generated');
      expect(output).toContain('struct IMUSensor');
      expect(output).toContain('std::string port');
      expect(output).toContain('int32_t rate');
      expect(output).toContain('Vec3f linear_acceleration');
      expect(output).toContain('Vec3f angular_velocity');
      expect(output).toContain('uint64_t timestamp');
    });

    it('handles array properties correctly', () => {
      const input = `
        export const JointEncoder = defineComponent({
          name: 'JointEncoder',
          props: {
            count: Schema.int32(),
          },
          state: {
            positions: Schema.array(Schema.float64()),
            velocities: Schema.array(Schema.float64()),
          }
        });
      `;

      const ast = parseRobotDefinition(input);
      const output = generator.generateComponent(ast.components[0]);

      expect(output).toContain('std::vector<double> positions');
      expect(output).toContain('std::vector<double> velocities');
    });
  });

  describe('Task Graph Generation', () => {
    it('generates YAML config for task dependencies', () => {
      const input = `
        const ReadSensors = defineTask({
          name: 'ReadSensors',
          outputs: { sensorData: SensorReading },
          timing: { period: Duration.ms(1) }
        });
        
        const ComputeControl = defineTask({
          name: 'ComputeControl',
          inputs: { sensorData: SensorReading },
          outputs: { command: MotorCommand },
          timing: { period: Duration.ms(1), deadline: Duration.us(800) }
        });
      `;

      const ast = parseRobotDefinition(input);
      const yaml = generator.generateTaskGraphYAML(ast.tasks);

      expect(yaml).toContain('name: ReadSensors');
      expect(yaml).toContain('name: ComputeControl');
      expect(yaml).toContain('depends_on:');
      expect(yaml).toContain('- ReadSensors');
      expect(yaml).toContain('period_ms: 1');
      expect(yaml).toContain('deadline_us: 800');
    });
  });

  describe('FlatBuffer Schema Generation', () => {
    it('generates valid FlatBuffer schema', () => {
      const input = `
        export const IMUReading = defineSchema({
          name: 'IMUReading',
          fields: {
            linearAcceleration: Schema.vec3(),
            angularVelocity: Schema.vec3(),
            timestampNs: Schema.uint64(),
          }
        });
      `;

      const ast = parseRobotDefinition(input);
      const fbs = generator.generateFlatBufferSchema(ast.schemas[0]);

      expect(fbs).toContain('namespace hefaos.generated;');
      expect(fbs).toContain('table IMUReading');
      expect(fbs).toContain('linear_acceleration:Vec3f');
      expect(fbs).toContain('angular_velocity:Vec3f');
      expect(fbs).toContain('timestamp_ns:uint64');
    });
  });
});
```

### 4.7 Snapshot/Golden File Tests

```typescript
// sdk/packages/compiler/tests/snapshots/simple-arm.test.ts

import { describe, it, expect } from 'vitest';
import { compile } from '../../src';
import { readFileSync, readdirSync } from 'fs';
import { join } from 'path';

const SNAPSHOTS_DIR = join(__dirname, 'snapshots');

describe('Compiler Snapshots', () => {
  const testCases = readdirSync(SNAPSHOTS_DIR).filter(
    f => readdirSync(join(SNAPSHOTS_DIR, f)).includes('input.tsx')
  );

  testCases.forEach(testCase => {
    describe(testCase, () => {
      const dir = join(SNAPSHOTS_DIR, testCase);
      const input = readFileSync(join(dir, 'input.tsx'), 'utf-8');

      it('generates expected C++ headers', async () => {
        const result = await compile(input, { format: 'cpp' });
        const expected = readFileSync(join(dir, 'expected.hpp'), 'utf-8');
        expect(result.cpp).toBe(expected);
      });

      it('generates expected YAML config', async () => {
        const result = await compile(input, { format: 'yaml' });
        const expected = readFileSync(join(dir, 'expected.yaml'), 'utf-8');
        expect(result.yaml).toBe(expected);
      });

      it('generates expected FlatBuffer schemas', async () => {
        const result = await compile(input, { format: 'flatbuffer' });
        const expected = readFileSync(join(dir, 'expected.fbs'), 'utf-8');
        expect(result.flatbuffer).toBe(expected);
      });
    });
  });
});
```

### 4.8 Simulation Integration Tests

```python
# simulator/tests/scenarios/test_pick_and_place.py

import pytest
import numpy as np
import mujoco
from hefaos_sim import HefaosSimulator, Robot, World
from hefaos_sim.assertions import (
    assert_reaches_position,
    assert_gripper_holds,
    assert_no_collision,
    assert_trajectory_smooth
)

@pytest.fixture
def simulator():
    """Create simulator with MuJoCo backend."""
    sim = HefaosSimulator(
        backend="mujoco",
        dt=0.001,  # 1kHz simulation
        render=False  # Headless for CI
    )
    yield sim
    sim.close()

@pytest.fixture
def arm_robot(simulator):
    """Load 7-DOF arm robot."""
    robot = Robot.from_urdf("robots/7dof_arm.urdf")
    simulator.add_robot(robot)
    return robot

@pytest.fixture
def world_with_object(simulator):
    """Create world with a graspable object."""
    world = World()
    world.add_box(
        name="target_object",
        position=[0.5, 0.0, 0.05],
        size=[0.05, 0.05, 0.05],
        mass=0.1
    )
    world.add_box(
        name="table",
        position=[0.5, 0.0, -0.025],
        size=[0.6, 0.4, 0.05],
        static=True
    )
    simulator.set_world(world)
    return world


class TestPickAndPlace:
    """End-to-end pick and place scenario tests."""

    def test_reach_object(self, simulator, arm_robot, world_with_object):
        """Robot can reach the target object."""
        target_pos = world_with_object.get_object("target_object").position
        
        # Command robot to move to pre-grasp position
        pre_grasp = target_pos + np.array([0, 0, 0.1])
        arm_robot.move_to_cartesian(pre_grasp, timeout=5.0)
        
        # Run simulation
        simulator.run(duration=5.0)
        
        # Assert
        assert_reaches_position(
            arm_robot.end_effector_position,
            pre_grasp,
            tolerance=0.01  # 1cm tolerance
        )

    def test_grasp_object(self, simulator, arm_robot, world_with_object):
        """Robot can grasp the object."""
        # Move to object
        target_pos = world_with_object.get_object("target_object").position
        arm_robot.move_to_cartesian(target_pos, timeout=5.0)
        simulator.run(duration=5.0)
        
        # Close gripper
        arm_robot.close_gripper()
        simulator.run(duration=1.0)
        
        # Assert gripper is holding object
        assert_gripper_holds(
            arm_robot.gripper,
            world_with_object.get_object("target_object")
        )

    def test_full_pick_and_place_cycle(self, simulator, arm_robot, world_with_object):
        """Complete pick and place cycle."""
        obj = world_with_object.get_object("target_object")
        pick_pos = obj.position.copy()
        place_pos = np.array([0.3, 0.2, 0.05])
        
        # Pick sequence
        arm_robot.move_to_cartesian(pick_pos + [0, 0, 0.1])
        simulator.run(duration=3.0)
        
        arm_robot.move_to_cartesian(pick_pos)
        simulator.run(duration=2.0)
        
        arm_robot.close_gripper()
        simulator.run(duration=0.5)
        
        arm_robot.move_to_cartesian(pick_pos + [0, 0, 0.1])
        simulator.run(duration=2.0)
        
        # Place sequence
        arm_robot.move_to_cartesian(place_pos + [0, 0, 0.1])
        simulator.run(duration=3.0)
        
        arm_robot.move_to_cartesian(place_pos)
        simulator.run(duration=2.0)
        
        arm_robot.open_gripper()
        simulator.run(duration=0.5)
        
        arm_robot.move_to_cartesian(place_pos + [0, 0, 0.1])
        simulator.run(duration=2.0)
        
        # Assert object is at target location
        final_obj_pos = obj.position
        np.testing.assert_allclose(
            final_obj_pos[:2],  # x, y
            place_pos[:2],
            atol=0.02  # 2cm tolerance
        )

    def test_no_self_collision(self, simulator, arm_robot, world_with_object):
        """Robot doesn't collide with itself during motion."""
        # Record collision events
        collisions = []
        simulator.on_collision(lambda a, b: collisions.append((a, b)))
        
        # Execute complex motion
        waypoints = [
            [0.4, 0.2, 0.3],
            [0.4, -0.2, 0.3],
            [0.6, 0.0, 0.1],
            [0.3, 0.0, 0.4],
        ]
        
        for wp in waypoints:
            arm_robot.move_to_cartesian(wp)
            simulator.run(duration=3.0)
        
        # Filter self-collisions (both bodies belong to same robot)
        self_collisions = [
            (a, b) for a, b in collisions
            if arm_robot.owns_body(a) and arm_robot.owns_body(b)
        ]
        
        assert len(self_collisions) == 0, f"Self collisions detected: {self_collisions}"

    def test_trajectory_smoothness(self, simulator, arm_robot, world_with_object):
        """Generated trajectories are smooth (no jerky motion)."""
        positions = []
        velocities = []
        
        def record_state():
            positions.append(arm_robot.joint_positions.copy())
            velocities.append(arm_robot.joint_velocities.copy())
        
        simulator.on_step(record_state)
        
        # Execute motion
        arm_robot.move_to_cartesian([0.5, 0.0, 0.2])
        simulator.run(duration=3.0)
        
        positions = np.array(positions)
        velocities = np.array(velocities)
        
        # Check smoothness via acceleration limits
        accelerations = np.diff(velocities, axis=0) / simulator.dt
        max_accel = np.max(np.abs(accelerations))
        
        assert max_accel < 50.0, f"Max acceleration {max_accel} exceeds limit"
        
        # Check no discontinuities in position
        position_jumps = np.diff(positions, axis=0)
        max_jump = np.max(np.abs(position_jumps))
        
        assert max_jump < 0.01, f"Position discontinuity detected: {max_jump}"


class TestSafetyBehaviors:
    """Test safety mechanisms and fault handling."""

    def test_emergency_stop(self, simulator, arm_robot):
        """Emergency stop halts all motion immediately."""
        # Start moving
        arm_robot.move_to_cartesian([0.5, 0.0, 0.3])
        simulator.run(duration=1.0)
        
        initial_velocity = np.linalg.norm(arm_robot.joint_velocities)
        assert initial_velocity > 0.1, "Robot should be moving"
        
        # Trigger e-stop
        arm_robot.emergency_stop()
        simulator.run(duration=0.1)  # Short time after e-stop
        
        # Assert velocities are near zero
        final_velocity = np.linalg.norm(arm_robot.joint_velocities)
        assert final_velocity < 0.01, f"Robot should have stopped: {final_velocity}"

    def test_joint_limit_protection(self, simulator, arm_robot):
        """Robot respects joint limits."""
        # Try to command beyond limits
        extreme_positions = arm_robot.joint_limits_upper + 0.5
        arm_robot.set_joint_positions(extreme_positions)
        simulator.run(duration=2.0)
        
        # Assert actual positions are within limits
        actual = arm_robot.joint_positions
        assert np.all(actual <= arm_robot.joint_limits_upper + 0.01)
        assert np.all(actual >= arm_robot.joint_limits_lower - 0.01)

    def test_watchdog_timeout(self, simulator, arm_robot):
        """Watchdog triggers on communication timeout."""
        # Configure watchdog
        arm_robot.set_watchdog(timeout_ms=100)
        
        # Start motion
        arm_robot.move_to_cartesian([0.5, 0.0, 0.3])
        simulator.run(duration=0.5)
        
        # Stop sending commands (simulate communication failure)
        arm_robot.disable_command_interface()
        
        # Wait for watchdog timeout
        simulator.run(duration=0.2)
        
        # Assert robot entered safe state
        assert arm_robot.state == "WATCHDOG_TRIGGERED"
        assert np.linalg.norm(arm_robot.joint_velocities) < 0.01
```

### 4.9 Property-Based Testing

```python
# simulator/tests/property/test_kinematics.py

import hypothesis
from hypothesis import given, strategies as st, settings
import numpy as np
from hefaos_sim import Robot

# Configure for robotics testing
settings.register_profile("robotics", max_examples=500, deadline=None)
settings.load_profile("robotics")


class TestKinematicsProperties:
    """Property-based tests for kinematics correctness."""

    @given(
        joint_positions=st.lists(
            st.floats(min_value=-np.pi, max_value=np.pi),
            min_size=7, max_size=7
        )
    )
    def test_forward_inverse_roundtrip(self, joint_positions):
        """FK(IK(FK(q))) ≈ FK(q) for valid configurations."""
        robot = Robot.from_urdf("robots/7dof_arm.urdf")
        q = np.array(joint_positions)
        
        # Skip invalid configurations
        if not robot.is_valid_configuration(q):
            return
        
        # Forward kinematics
        pose1 = robot.forward_kinematics(q)
        
        # Inverse kinematics (may have multiple solutions)
        q_ik = robot.inverse_kinematics(pose1)
        if q_ik is None:
            return  # No IK solution
        
        # Forward kinematics again
        pose2 = robot.forward_kinematics(q_ik)
        
        # Poses should match
        np.testing.assert_allclose(pose1.position, pose2.position, atol=1e-6)
        # Quaternion comparison (handle sign ambiguity)
        dot = abs(np.dot(pose1.orientation, pose2.orientation))
        assert dot > 0.9999, f"Orientation mismatch: {dot}"

    @given(
        q=st.lists(st.floats(-np.pi, np.pi), min_size=7, max_size=7),
        dq=st.lists(st.floats(-1.0, 1.0), min_size=7, max_size=7),
    )
    def test_jacobian_numerical_consistency(self, q, dq):
        """Analytical Jacobian matches numerical differentiation."""
        robot = Robot.from_urdf("robots/7dof_arm.urdf")
        q = np.array(q)
        dq = np.array(dq)
        
        if not robot.is_valid_configuration(q):
            return
        
        # Analytical Jacobian
        J = robot.jacobian(q)
        dx_analytical = J @ dq
        
        # Numerical Jacobian
        eps = 1e-6
        pose_0 = robot.forward_kinematics(q)
        
        dx_numerical = np.zeros(6)
        for i in range(7):
            q_plus = q.copy()
            q_plus[i] += eps * dq[i]
            pose_plus = robot.forward_kinematics(q_plus)
            
            dx_numerical[:3] += (pose_plus.position - pose_0.position) / eps
        
        # Compare (position part only for simplicity)
        np.testing.assert_allclose(
            dx_analytical[:3], 
            J[:3, :] @ dq, 
            atol=1e-4
        )

    @given(
        base_pose=st.tuples(
            st.floats(-10, 10), st.floats(-10, 10), st.floats(0, 2)
        )
    )
    def test_transform_chain_associativity(self, base_pose):
        """Transform composition is associative."""
        from hefaos_sim.math import Transform
        
        T1 = Transform.from_xyz(*base_pose)
        T2 = Transform.from_rotation_z(0.5)
        T3 = Transform.from_xyz(1, 0, 0)
        
        # (T1 * T2) * T3 = T1 * (T2 * T3)
        result1 = (T1 @ T2) @ T3
        result2 = T1 @ (T2 @ T3)
        
        np.testing.assert_allclose(
            result1.as_matrix(),
            result2.as_matrix(),
            atol=1e-10
        )
```

### 4.10 Hardware-in-the-Loop (HIL) Tests

```cpp
// runtime/tests/hil/test_motor_controller.cpp

#include <gtest/gtest.h>
#include <hefaos/hal/can.hpp>
#include <hefaos/hal/motor_controller.hpp>

namespace hefaos::test::hil {

// These tests require actual hardware connected
// Run with: ctest -L hil

class MotorControllerHILTest : public ::testing::Test {
protected:
    void SetUp() override {
        // Skip if no CAN interface
        if (!CANBus::interface_exists("can0")) {
            GTEST_SKIP() << "No CAN interface available";
        }
        
        can_ = std::make_unique<CANBus>("can0");
        motor_ = std::make_unique<MotorController>(can_.get(), 0x01);
        
        // Enable motor
        ASSERT_TRUE(motor_->enable());
    }
    
    void TearDown() override {
        if (motor_) {
            motor_->disable();
        }
    }
    
    std::unique_ptr<CANBus> can_;
    std::unique_ptr<MotorController> motor_;
};

TEST_F(MotorControllerHILTest, ReadEncoderPosition) {
    auto position = motor_->read_position();
    ASSERT_TRUE(position.has_value());
    
    // Position should be within valid range
    EXPECT_GE(*position, -M_PI);
    EXPECT_LE(*position, M_PI);
}

TEST_F(MotorControllerHILTest, PositionControl) {
    constexpr float target = 0.5f;  // radians
    constexpr float tolerance = 0.02f;  // 20mrad
    
    motor_->set_position(target);
    
    // Wait for motion to complete (max 5 seconds)
    auto start = std::chrono::steady_clock::now();
    while (std::chrono::steady_clock::now() - start < std::chrono::seconds(5)) {
        auto pos = motor_->read_position();
        if (pos && std::abs(*pos - target) < tolerance) {
            SUCCEED();
            return;
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    
    auto final_pos = motor_->read_position();
    FAIL() << "Position " << *final_pos << " didn't reach target " << target;
}

TEST_F(MotorControllerHILTest, VelocityControl) {
    constexpr float target_vel = 1.0f;  // rad/s
    constexpr float tolerance = 0.1f;
    
    motor_->set_velocity(target_vel);
    
    // Let motor reach steady state
    std::this_thread::sleep_for(std::chrono::milliseconds(500));
    
    auto velocity = motor_->read_velocity();
    ASSERT_TRUE(velocity.has_value());
    EXPECT_NEAR(*velocity, target_vel, tolerance);
    
    // Stop motor
    motor_->set_velocity(0.0f);
}

TEST_F(MotorControllerHILTest, TorqueLimit) {
    constexpr float max_torque = 2.0f;  // Nm
    
    motor_->set_torque_limit(max_torque);
    motor_->set_torque(10.0f);  // Request more than limit
    
    std::this_thread::sleep_for(std::chrono::milliseconds(100));
    
    auto actual_torque = motor_->read_torque();
    ASSERT_TRUE(actual_torque.has_value());
    EXPECT_LE(*actual_torque, max_torque + 0.1f);
}

TEST_F(MotorControllerHILTest, EmergencyStop) {
    // Start motion
    motor_->set_velocity(2.0f);
    std::this_thread::sleep_for(std::chrono::milliseconds(200));
    
    // Trigger e-stop
    motor_->emergency_stop();
    
    // Velocity should drop quickly
    std::this_thread::sleep_for(std::chrono::milliseconds(50));
    auto velocity = motor_->read_velocity();
    
    ASSERT_TRUE(velocity.has_value());
    EXPECT_LT(std::abs(*velocity), 0.1f);
}

}  // namespace hefaos::test::hil
```

### 4.11 Test Configuration & CI Integration

For a solo developer, a single unified CI workflow is simpler to maintain. Use path filters to run only relevant jobs.

```yaml
# .github/workflows/ci.yml

name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  BUILD_TYPE: Release

jobs:
  # ─────────────────────────────────────────────────────────────────────
  # C++ Runtime (only runs when runtime/** changes)
  # ─────────────────────────────────────────────────────────────────────
  runtime:
    runs-on: ubuntu-24.04
    if: |
      github.event_name == 'push' ||
      contains(github.event.pull_request.changed_files, 'runtime/')
    
    steps:
      - uses: actions/checkout@v4
        with:
          submodules: recursive
      
      - name: Cache iceoryx
        id: cache-iceoryx
        uses: actions/cache@v4
        with:
          path: ~/iceoryx
          key: iceoryx-2.0.5-${{ runner.os }}
      
      - name: Install dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            cmake ninja-build clang-18 \
            libboost-all-dev libtbb-dev \
            libarrow-dev libflatbuffers-dev \
            libgtest-dev libgmock-dev
      
      - name: Install iceoryx
        if: steps.cache-iceoryx.outputs.cache-hit != 'true'
        run: |
          git clone https://github.com/eclipse-iceoryx/iceoryx.git --depth 1 --branch v2.0.5
          cd iceoryx
          cmake -B build -G Ninja -DCMAKE_INSTALL_PREFIX=$HOME/iceoryx
          cmake --build build --parallel
          cmake --install build
      
      - name: Build & Test
        run: |
          export CMAKE_PREFIX_PATH=$HOME/iceoryx
          cd runtime
          cmake -B build -G Ninja -DCMAKE_BUILD_TYPE=Release -DBUILD_TESTING=ON
          cmake --build build --parallel
          ctest --test-dir build --output-on-failure

  # ─────────────────────────────────────────────────────────────────────
  # TypeScript SDK (only runs when sdk/** changes)
  # ─────────────────────────────────────────────────────────────────────
  sdk:
    runs-on: ubuntu-latest
    if: |
      github.event_name == 'push' ||
      contains(github.event.pull_request.changed_files, 'sdk/')
    
    steps:
      - uses: actions/checkout@v4
      
      - uses: pnpm/action-setup@v2
        with:
          version: 9
      
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'pnpm'
          cache-dependency-path: sdk/pnpm-lock.yaml
      
      - name: Install, Build & Test
        run: |
          cd sdk
          pnpm install
          pnpm typecheck
          pnpm lint
          pnpm build
          pnpm test

  # ─────────────────────────────────────────────────────────────────────
  # Quick checks (always runs)
  # ─────────────────────────────────────────────────────────────────────
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Check C++ formatting
        run: |
          find runtime -name '*.cpp' -o -name '*.hpp' | head -20 | \
            xargs clang-format --dry-run --Werror 2>/dev/null || true
      
      - uses: pnpm/action-setup@v2
        with:
          version: 9
      
      - name: Check TypeScript
        run: |
          cd sdk && pnpm install && pnpm lint
```

**Note:** For more complex setups later, you can split this into `ci-runtime.yml` and `ci-sdk.yml` with independent triggers. The single file approach is simpler for now.

---

## 5. CI/CD Pipeline

### 5.1 Pipeline Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            CI/CD Pipeline                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────────┐    ┌────────────┐    ┌────────────┐    ┌────────────┐      │
│  │   Commit   │───►│   Build    │───►│    Test    │───►│  Release   │      │
│  │            │    │            │    │            │    │            │      │
│  └────────────┘    └────────────┘    └────────────┘    └────────────┘      │
│        │                 │                 │                 │              │
│        ▼                 ▼                 ▼                 ▼              │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                                                                       │  │
│  │  Lint          x86_64 Build      Unit Tests        Tag & Version     │  │
│  │  Format        ARM64 Build       Component Tests   Changelog         │  │
│  │  Type Check    WASM Build        Integration       GitHub Release    │  │
│  │                                  Simulation        npm Publish       │  │
│  │                                  Benchmarks        Docker Push       │  │
│  │                                                                       │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Test Execution Matrix

| Test Type | Trigger | Duration | Required for Merge |
|-----------|---------|----------|-------------------|
| Lint & Format | All PRs | <1 min | ✅ Yes |
| Type Check | All PRs | <2 min | ✅ Yes |
| Unit Tests | All PRs | <5 min | ✅ Yes |
| Component Tests | All PRs | <10 min | ✅ Yes |
| Integration Tests | All PRs | <15 min | ✅ Yes |
| Simulation Tests | PRs + main | <30 min | ⚠️ main only |
| HIL Tests | Nightly | <60 min | ❌ No |
| Benchmarks | Weekly | <120 min | ❌ No |

---

## 6. Quick Start Guide

### For Windows Developers

```powershell
# 1. Run setup (as Administrator)
.\tools\scripts\setup-dev.ps1

# 2. Restart computer

# 3. Open Ubuntu and run WSL setup
wsl bash ~/setup-hefaos-wsl.sh

# 4. Clone repo (in WSL)
cd ~/hefaos-dev
git clone https://github.com/hefaos-robotics/hefaos.git
cd hefaos

# 5. Open in VS Code
code .

# 6. Build C++ runtime
cd runtime
cmake -B build -G Ninja
cmake --build build --parallel

# 7. Build TypeScript SDK
cd ../sdk
pnpm install
pnpm build

# 8. Run tests
cd ../runtime && ctest --test-dir build
cd ../sdk && pnpm test
```

### Using DevContainer (Recommended)

```bash
# 1. Clone repo
git clone https://github.com/hefaos-robotics/hefaos.git
cd hefaos

# 2. Open in VS Code
code .

# 3. When prompted, click "Reopen in Container"
#    Or: Ctrl+Shift+P → "Dev Containers: Reopen in Container"

# 4. Wait for container build (first time ~10 min)

# 5. Development environment is ready!
```

---

## Summary

This document provides:

1. **Single Monorepo Strategy**: Everything in one repo for simplicity — split later when pain points emerge (models >500MB, community contributions, independent release cycles)
2. **Windows Dev Environment**: WSL2 + Docker Desktop + VS Code with full cross-compilation support
3. **Comprehensive Testing**: Unit → Component → Integration → Simulation → HIL pyramid with timing verification
4. **Simplified CI/CD**: Single GitHub Actions workflow with path filters to run only what's needed

The testing strategy specifically addresses the unique requirements of robotics software: timing verification, physics simulation, safety behaviors, and property-based testing for mathematical correctness.

### When to Evolve This Setup

| Sign | Action |
|------|--------|
| Models exceed 500MB | Extract `models/` to separate repo with Git LFS |
| Community wants to contribute examples | Extract `examples/` to public repo |
| Board support needs independent releases | Extract `boards/` to separate repo |
| CI takes >30 min on every PR | Split into `ci-runtime.yml` and `ci-sdk.yml` |
| Team grows beyond 5 people | Consider federated monorepo structure |

Until then, keep it simple and ship. 🚀
