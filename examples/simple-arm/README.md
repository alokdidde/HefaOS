# Simple Arm Example

A 6-DOF robotic arm example demonstrating the Hefaos SDK.

## Overview

This example shows how to:

- Define robot components (joints, sensors)
- Configure real-time tasks with priorities and deadlines
- Create behavior trees for autonomous operation

## Robot Specification

| Property | Value |
|----------|-------|
| DOF | 6 (revolute joints) |
| Control Rate | 1 kHz |
| Control Deadline | 800 μs |
| Sensors | IMU, Force/Torque |

## File Structure

```
simple-arm/
├── robot.tsx        # Robot definition
├── README.md        # This file
└── behaviors/       # Additional behaviors (optional)
```

## Usage

### Build

```bash
# Compile robot definition
cd sdk
pnpm build
npx hefaos build ../examples/simple-arm

# Output: generated C++ code and configuration
```

### Simulate

```bash
# Run in MuJoCo simulator
npx hefaos simulate ../examples/simple-arm
```

### Deploy

```bash
# Deploy to robot hardware
npx hefaos deploy ../examples/simple-arm --target 192.168.1.100
```

## Task Graph

```
ReadSensors ─────┬─────> ComputeControl ────> WriteActuators
                 │
                 ├─────> SafetyMonitor
                 │
PlanMotion ──────┘

PublishTelemetry (independent, low priority)
```

## Behaviors

### Idle

Holds current position. This is the default behavior.

### Pick and Place

Executes a pick and place operation:

1. Move to pick approach pose
2. Move to pick pose
3. Close gripper
4. Verify grasp
5. Retreat
6. Move to place approach
7. Move to place pose
8. Open gripper
9. Retreat
10. Return home

### Homing

Moves all joints to their home position and zeros encoders.

## Customization

### Adding a New Joint

```tsx
<Joint
  name="custom_joint"
  type="revolute"
  limits={[-90, 90]}
  maxVelocity={180}
  maxTorque={50}
/>
```

### Adding a New Task

```tsx
<Task
  name="MyCustomTask"
  priority="high"
  period={Duration.ms(10)}
  inputs={['joint_states']}
  outputs={['custom_output']}
/>
```

### Creating a New Behavior

```tsx
export const MyBehavior = defineBehavior({
  name: 'my_behavior',
  tree: (
    <Sequence>
      <Action name="Step1" />
      <Action name="Step2" />
    </Sequence>
  ),
});
```

## Hardware Requirements

- 6 motor drivers with EtherCAT or CAN interface
- IMU sensor (connected via USB/serial)
- Force/torque sensor (connected via USB/serial)
- Linux system with RT_PREEMPT kernel (for real-time)

## License

MIT License - See [LICENSE](../../LICENSE)
