# HefaOS

**An open stack for building real robots.**

A modern robotics framework combining real-time C++ performance with TypeScript developer experience.

## Overview

Hefaos provides:

- **Real-time Runtime** - C++ core with deterministic scheduling, zero-copy IPC via iceoryx, and sub-millisecond control loops
- **TypeScript SDK** - React-inspired component model for defining robots, behaviors, and tasks
- **Cross-platform Simulation** - MuJoCo/Gazebo integration for development and testing
- **AI Integration** - ONNX Runtime, TensorFlow Lite, and llama.cpp backends for perception and planning

## Repository Structure

```
hefaos/
├── runtime/          # C++ real-time components
│   ├── core/         # State store, task graph, executor
│   ├── hal/          # Hardware abstraction layer
│   └── ai/           # AI runtime backends
├── sdk/              # TypeScript SDK and tools
│   └── packages/
│       ├── sdk/      # @hefaos/sdk - Component definitions
│       ├── compiler/ # @hefaos/compiler - TSX to C++ compiler
│       ├── cli/      # @hefaos/cli - Development CLI
│       └── types/    # @hefaos/types - Shared type definitions
├── simulator/        # Simulation backends
├── boards/           # Board support packages
├── models/           # Pre-trained AI models (gitignored)
├── examples/         # Example robot projects
├── docs/             # Documentation
└── tools/            # Development tools and scripts
```

## Quick Start

### Prerequisites

- **Windows**: WSL2 with Ubuntu 24.04, Docker Desktop
- **Linux**: Ubuntu 24.04 or equivalent
- **macOS**: Homebrew, Docker Desktop

### Development Setup

```bash
# Clone the repository
git clone https://github.com/alokdidde/HefaOS.git
cd HefaOS

# Build C++ runtime
cd runtime
cmake -B build -G Ninja
cmake --build build --parallel

# Build TypeScript SDK
cd ../sdk
pnpm install
pnpm build

# Run tests
cd ../runtime && ctest --test-dir build
cd ../sdk && pnpm test
```

### Using DevContainer (Recommended)

1. Install VS Code with Remote - Containers extension
2. Open the repository folder
3. Click "Reopen in Container" when prompted
4. Wait for the container to build

## Example: Simple Arm Robot

```tsx
// examples/simple-arm/robot.tsx
import { defineRobot, Arm, Joint, IMU, Task, Duration } from '@hefaos/sdk';

export default defineRobot({
  name: 'SimpleArm',
  components: [
    <Arm name="arm" dof={6}>
      <Joint name="base" type="revolute" limits={[-180, 180]} />
      <Joint name="shoulder" type="revolute" limits={[-90, 90]} />
      <Joint name="elbow" type="revolute" limits={[-120, 120]} />
      <Joint name="wrist1" type="revolute" limits={[-180, 180]} />
      <Joint name="wrist2" type="revolute" limits={[-90, 90]} />
      <Joint name="wrist3" type="revolute" limits={[-180, 180]} />
    </Arm>,
    <IMU name="imu" port="/dev/ttyUSB0" rate={1000} />
  ],
  tasks: [
    <Task name="ReadSensors" period={Duration.ms(1)} />,
    <Task name="ComputeControl" period={Duration.ms(1)} deadline={Duration.us(800)} />,
    <Task name="WriteActuators" period={Duration.ms(1)} />
  ]
});
```

## Documentation

- [Getting Started](docs/getting-started/)
- [Architecture](docs/architecture/)
- [API Reference](docs/api/)
- [Tutorials](docs/tutorials/)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT License - see [LICENSE](LICENSE) for details.
