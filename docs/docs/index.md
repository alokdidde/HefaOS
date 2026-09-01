# Hefaos Documentation

**Hardware-Efficient Framework for Autonomous Operating Systems**

Welcome to the Hefaos documentation. Hefaos is a modern robotics framework that combines real-time C++ performance with a TypeScript developer experience.

## What is Hefaos?

Hefaos provides:

- **Real-time Runtime** - C++ core with deterministic scheduling, zero-copy IPC via iceoryx, and sub-millisecond control loops
- **TypeScript SDK** - React-inspired component model for defining robots, behaviors, and tasks
- **Cross-platform Simulation** - MuJoCo/Gazebo integration for development and testing
- **AI Integration** - ONNX Runtime, TensorFlow Lite, and llama.cpp backends for perception and planning

## Quick Start

```bash
# Clone the repository
git clone https://github.com/hefaos-robotics/hefaos.git
cd hefaos

# Build C++ runtime
cd runtime
cmake -B build -G Ninja
cmake --build build --parallel

# Build TypeScript SDK
cd ../sdk
pnpm install
pnpm build
```

## Example

```tsx
import { defineRobot, Arm, Joint, IMU, Task, Duration } from '@hefaos/sdk';

export default defineRobot({
  name: 'SimpleArm',
  components: [
    <Arm name="arm" dof={6}>
      <Joint name="base" type="revolute" limits={[-180, 180]} />
      <Joint name="shoulder" type="revolute" limits={[-90, 90]} />
      <Joint name="elbow" type="revolute" limits={[-120, 120]} />
    </Arm>,
    <IMU name="imu" port="/dev/ttyUSB0" rate={1000} />
  ],
  tasks: [
    <Task name="ReadSensors" period={Duration.ms(1)} />,
    <Task name="ComputeControl" period={Duration.ms(1)} deadline={Duration.us(800)} />
  ]
});
```

## Next Steps

- [Installation Guide](getting-started/installation.md)
- [Quick Start Tutorial](getting-started/quick-start.md)
- [Architecture Overview](architecture/overview.md)
