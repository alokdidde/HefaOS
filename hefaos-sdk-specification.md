# HefaOS SDK: Developer Experience Layer

## TypeScript/React-Style Robot Development

**Version:** 1.0.0  
**Status:** Draft  
**Last Updated:** January 2026

---

## Table of Contents

1. [Overview](#1-overview)
2. [Architecture](#2-architecture)
3. [Alternative Approaches Considered](#3-alternative-approaches-considered)
4. [Core Concepts](#4-core-concepts)
5. [Component System](#5-component-system)
6. [Task Graph Definition](#6-task-graph-definition)
7. [Robot Composition](#7-robot-composition)
8. [Behavior Trees](#8-behavior-trees)
9. [AI Model Integration](#9-ai-model-integration)
10. [State Management](#10-state-management)
11. [Hooks and Lifecycle](#11-hooks-and-lifecycle)
12. [Type System](#12-type-system)
13. [Code Generation](#13-code-generation)
14. [Development Workflow](#14-development-workflow)
15. [Tooling](#15-tooling)
16. [Examples](#16-examples)

---

## 1. Overview

### 1.1 Vision

HefaOS SDK enables robotics developers to define robot components, behaviors, and task graphs using familiar TypeScript and JSX syntax, while the HefaOS C++ runtime handles all performance-critical execution.

```tsx
// This is what robot development looks like with HefaOS SDK
import { defineRobot, Component, Task, useRobotState } from '@hefaos/sdk';

const MyRobot = defineRobot({
  name: 'Manipulator',
  
  components: (
    <>
      <IMUSensor port="/dev/spi0" rate={1000} />
      <JointEncoder count={7} type="absolute" />
      <RGBCamera resolution={[640, 480]} fps={30} />
      <GripperController type="parallel" />
    </>
  ),
  
  tasks: (
    <TaskGraph>
      <ControlLoop rate="1kHz" priority="realtime" />
      <PerceptionPipeline rate="30Hz" />
      <PlanningLoop rate="10Hz" />
    </TaskGraph>
  ),
  
  models: {
    objectDetection: model('yolov8s.onnx', { priority: 'perception' }),
    graspPolicy: model('grasp_policy.tflite', { priority: 'control' }),
  },
  
  behavior: <PickAndPlaceBehavior />,
});

export default MyRobot;
```

### 1.2 Goals

1. **Familiar Syntax:** Leverage TypeScript and JSX that millions of developers already know
2. **Type Safety:** Full type inference and compile-time validation
3. **Declarative:** Describe *what* the robot does, not *how* it executes
4. **Zero Runtime Overhead:** Everything compiles to native C++ and configuration
5. **Hot Reload:** Rapid iteration during development (simulation mode)
6. **Visual Tools:** Component graphs, task timing, state inspection

### 1.3 Non-Goals

- Runtime TypeScript execution on the robot (performance critical paths are C++)
- Replacing C++ for custom drivers or performance-critical algorithms
- Full React reconciliation (robots aren't UIs with frequent re-renders)

---

## 2. Architecture

### 2.1 System Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        Development Time                                  │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌────────────────────────────────────────────────────────────────┐    │
│   │                    TypeScript/JSX Source                        │    │
│   │                                                                 │    │
│   │   robot.tsx          components/*.tsx       behaviors/*.tsx     │    │
│   │   tasks.tsx          models.tsx             config.ts           │    │
│   └────────────────────────────────────────────────────────────────┘    │
│                                    │                                     │
│                           hefaos-compiler                                 │
│                                    │                                     │
│   ┌────────────────────────────────┼────────────────────────────────┐   │
│   │                                ▼                                 │   │
│   │   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐         │   │
│   │   │ C++ Headers  │  │ FlatBuffer   │  │ Task Graph   │         │   │
│   │   │ & Sources    │  │ Schemas      │  │ Config       │         │   │
│   │   │              │  │              │  │              │         │   │
│   │   │ components/  │  │ schemas/     │  │ graphs/      │         │   │
│   │   │ ├─ imu.hpp   │  │ ├─ imu.fbs   │  │ ├─ control   │         │   │
│   │   │ ├─ joint.hpp │  │ ├─ joint.fbs │  │ │   .yaml    │         │   │
│   │   │ └─ ...       │  │ └─ ...       │  │ └─ ...       │         │   │
│   │   └──────────────┘  └──────────────┘  └──────────────┘         │   │
│   │                                                                 │   │
│   │   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐         │   │
│   │   │ TypeScript   │  │ Behavior     │  │ Model        │         │   │
│   │   │ Bindings     │  │ Trees        │  │ Configs      │         │   │
│   │   │              │  │              │  │              │         │   │
│   │   │ types/       │  │ behaviors/   │  │ models/      │         │   │
│   │   │ ├─ index.d.ts│  │ ├─ pick.xml  │  │ ├─ detect    │         │   │
│   │   │ └─ ...       │  │ └─ ...       │  │ │   .yaml    │         │   │
│   │   └──────────────┘  └──────────────┘  └──────────────┘         │   │
│   │                                                                 │   │
│   └─────────────────────── Generated Artifacts ─────────────────────┘   │
│                                    │                                     │
└────────────────────────────────────┼─────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                          Runtime                                         │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌────────────────────────────────────────────────────────────────┐    │
│   │                    HefaOS C++ Runtime                            │    │
│   │                                                                 │    │
│   │   • Loads generated configurations                             │    │
│   │   • Instantiates components from C++ registry                  │    │
│   │   • Builds task graph from generated definitions               │    │
│   │   • Executes with full real-time performance                   │    │
│   └────────────────────────────────────────────────────────────────┘    │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Compilation Pipeline

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  TypeScript │────►│   Parse &   │────►│  Validate   │────►│  Generate   │
│    Source   │     │  Transform  │     │   & Check   │     │  Artifacts  │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
                                                                    │
                    ┌───────────────────────────────────────────────┘
                    │
                    ▼
    ┌───────────────────────────────────────────────────────────────────┐
    │                      Generated Outputs                             │
    ├───────────────────────────────────────────────────────────────────┤
    │                                                                    │
    │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐               │
    │  │    C++      │  │ FlatBuffer  │  │   YAML      │               │
    │  │  Headers    │  │  Schemas    │  │  Configs    │               │
    │  └─────────────┘  └─────────────┘  └─────────────┘               │
    │                                                                    │
    │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐               │
    │  │  Behavior   │  │   Model     │  │  Type       │               │
    │  │   Trees     │  │  Manifests  │  │ Definitions │               │
    │  └─────────────┘  └─────────────┘  └─────────────┘               │
    │                                                                    │
    └───────────────────────────────────────────────────────────────────┘
```

---

## 3. Alternative Approaches Considered

### 3.1 Comparison Matrix

| Approach | Developer Experience | Performance | Type Safety | Ecosystem | Recommended |
|----------|---------------------|-------------|-------------|-----------|-------------|
| **TypeScript + JSX** | ★★★★★ | ★★★★★ (compiled) | ★★★★★ | ★★★★★ | ✅ Primary |
| Rust DSL | ★★★☆☆ | ★★★★★ | ★★★★★ | ★★★☆☆ | For power users |
| Lua/Luau | ★★★★☆ | ★★★★☆ | ★★☆☆☆ | ★★★☆☆ | For scripting |
| YAML/TOML | ★★★☆☆ | ★★★★★ | ★★☆☆☆ | ★★★★☆ | Config only |
| Python DSL | ★★★★☆ | ★★☆☆☆ | ★★★☆☆ | ★★★★★ | Prototyping |
| Visual Programming | ★★★★★ | ★★★★☆ | ★★★☆☆ | ★★☆☆☆ | Non-programmers |

### 3.2 Why TypeScript + JSX

**Pros:**
- Largest developer talent pool (millions of JS/TS developers)
- Excellent tooling (VS Code, ESLint, Prettier)
- JSX is perfect for declarative composition
- Strong type system with inference
- Can compile to anything (C++, config files)
- Hot Module Replacement for rapid iteration

**Cons:**
- Not the "traditional" robotics language
- Requires compilation step
- Some developers prefer Rust/C++ directly

**Mitigation:**
- Provide escape hatches to write C++ directly
- Generate readable C++ that can be maintained independently
- Support multiple frontend languages (Rust DSL planned)

### 3.3 Alternative: Solid.js Style (Fine-Grained Reactivity)

Instead of React's component model, we could use Solid.js-style fine-grained reactivity:

```tsx
// Solid.js style - more natural for robotics signals
import { createSignal, createEffect } from '@hefaos/reactive';

const [imuReading, setIMU] = createSignal<IMUReading>();
const [jointState, setJoints] = createSignal<JointState>();

// Derived state (automatically updates)
const stateEstimate = createMemo(() => 
  kalmanFilter(imuReading(), jointState())
);

// Side effect (runs when dependencies change)
createEffect(() => {
  const cmd = controller.compute(stateEstimate());
  motorDriver.send(cmd);
});
```

**Recommendation:** Support both styles:
- JSX/Component style for high-level robot definition
- Signals/Reactive style for complex state management within behaviors

---

## 4. Core Concepts

### 4.1 Mental Model

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         HefaOS SDK Concepts                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   COMPONENTS                    TASKS                     BEHAVIORS      │
│   ───────────                   ─────                     ─────────      │
│   What the robot               How computations           What the       │
│   is made of                   are scheduled              robot does     │
│                                                                          │
│   ┌─────────────┐              ┌─────────────┐           ┌───────────┐  │
│   │ <IMUSensor/>│              │ ReadIMU     │           │   Pick    │  │
│   │ <Camera/>   │──────────────│     │       │───────────│     │     │  │
│   │ <Gripper/>  │              │ Estimate    │           │   Place   │  │
│   │ <Motor/>    │              │     │       │           │     │     │  │
│   └─────────────┘              │ Control     │           │   Done    │  │
│                                └─────────────┘           └───────────┘  │
│                                                                          │
│   MODELS                       STATE                      SIGNALS        │
│   ──────                       ─────                      ───────        │
│   AI/ML models                 Shared data                Reactive       │
│   for inference                between tasks              data flow      │
│                                                                          │
│   ┌─────────────┐              ┌─────────────┐           ┌───────────┐  │
│   │ YOLOv8      │              │ StateStore  │           │ imu$      │  │
│   │ GraspPolicy │──────────────│ {           │───────────│ joints$   │  │
│   │ LLM Agent   │              │   imu,      │           │ estimate$ │  │
│   └─────────────┘              │   joints,   │           │ command$  │  │
│                                │   estimate  │           └───────────┘  │
│                                │ }           │                          │
│                                └─────────────┘                          │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Package Structure

```
@hefaos/sdk
├── core/           # Core definitions and types
├── components/     # Built-in component definitions
├── tasks/          # Task graph primitives
├── behaviors/      # Behavior tree nodes
├── models/         # AI model integration
├── state/          # State management (signals, stores)
├── hooks/          # Lifecycle hooks
├── compiler/       # TypeScript to artifacts compiler
└── devtools/       # Development tools and visualizers

@hefaos/components   # Standard component library
├── sensors/        # IMU, camera, lidar, etc.
├── actuators/      # Motors, grippers, etc.
├── drivers/        # Hardware driver bindings
└── utils/          # Transform, filter, etc.

@hefaos/behaviors    # Behavior tree library
├── actions/        # Primitive actions
├── conditions/     # Condition nodes
├── composites/     # Sequence, selector, parallel
└── decorators/     # Retry, timeout, etc.
```

---

## 5. Component System

### 5.1 Defining Components

Components describe data structures and their associated hardware or computation.

```tsx
// Basic component definition
import { defineComponent, Schema } from '@hefaos/sdk';

export const IMUReading = defineComponent({
  name: 'IMUReading',
  
  // Schema defines the data structure
  schema: Schema.object({
    linearAcceleration: Schema.vec3(),
    angularVelocity: Schema.vec3(),
    timestamp: Schema.uint64(),
    sequence: Schema.uint32(),
  }),
  
  // Optional metadata
  metadata: {
    description: 'Inertial Measurement Unit reading',
    unit: { linearAcceleration: 'm/s²', angularVelocity: 'rad/s' },
    coordinateFrame: 'imu_link',
  },
});

// Component with validation
export const JointState = defineComponent({
  name: 'JointState',
  
  schema: Schema.object({
    position: Schema.float64(),
    velocity: Schema.float64(),
    effort: Schema.float64(),
    timestamp: Schema.uint64(),
  }),
  
  // Validation rules
  validate: (value) => ({
    position: value.position >= -Math.PI && value.position <= Math.PI,
    velocity: Math.abs(value.velocity) <= 10.0,  // rad/s limit
    effort: Math.abs(value.effort) <= 100.0,     // Nm limit
  }),
});
```

### 5.2 Hardware-Backed Components

```tsx
// Sensor component with hardware binding
import { defineSensor, Schema, HardwareBinding } from '@hefaos/sdk';

export const BMI088 = defineSensor({
  name: 'BMI088',
  extends: 'IMUSensor',  // Inherits IMU interface
  
  // Hardware configuration
  hardware: HardwareBinding.spi({
    device: Schema.string(),      // e.g., "/dev/spidev0.0"
    speed: Schema.number(),       // Hz
    mode: Schema.enum([0, 1, 2, 3]),
  }),
  
  // Configurable parameters
  config: {
    sampleRate: Schema.number().default(1000).min(100).max(2000),
    accelRange: Schema.enum(['2g', '4g', '8g', '16g']).default('4g'),
    gyroRange: Schema.enum(['250dps', '500dps', '1000dps', '2000dps']).default('500dps'),
  },
  
  // Output component type
  outputs: {
    reading: IMUReading,
  },
  
  // Maps to C++ driver class
  driver: 'hefaos::drivers::BMI088Driver',
});

// Usage in robot definition
<BMI088 
  device="/dev/spidev0.0" 
  sampleRate={1000}
  accelRange="4g"
  gyroRange="500dps"
/>
```

### 5.3 Computed Components

```tsx
// Component that derives from other components
import { defineComputed, Schema } from '@hefaos/sdk';

export const StateEstimate = defineComputed({
  name: 'StateEstimate',
  
  // Input dependencies
  inputs: {
    imu: IMUReading,
    joints: JointState.array(7),
    lastEstimate: Schema.optional(StateEstimate),
  },
  
  // Output schema
  schema: Schema.object({
    pose: Schema.pose(),
    twist: Schema.twist(),
    covariance: Schema.matrix(6, 6),
    timestamp: Schema.uint64(),
  }),
  
  // Computation can be:
  // 1. Reference to C++ function
  compute: 'hefaos::estimation::ExtendedKalmanFilter',
  
  // 2. Or inline expression (compiled to C++)
  // compute: (inputs) => {
  //   return kalmanUpdate(inputs.lastEstimate, inputs.imu, inputs.joints);
  // },
});
```

### 5.4 Component Composition (JSX Style)

```tsx
// Composing components into a sensor suite
import { ComponentGroup } from '@hefaos/sdk';

export const SensorSuite = () => (
  <ComponentGroup name="sensors">
    <BMI088 
      name="imu"
      device="/dev/spidev0.0" 
      sampleRate={1000}
    />
    
    <AS5048A
      name="encoders"
      device="/dev/spidev0.1"
      joints={7}
    />
    
    <RealSenseD435
      name="wrist_camera"
      serial="12345678"
      resolution={[640, 480]}
      fps={30}
      enableDepth={true}
    />
    
    <ATIMini45
      name="wrist_ft"
      device="/dev/ttyUSB0"
      calibration="FT12345.cal"
    />
  </ComponentGroup>
);

// Composing actuators
export const ActuatorSuite = () => (
  <ComponentGroup name="actuators">
    <CyberGearMotor
      name="joints"
      canInterface="can0"
      motorIds={[1, 2, 3, 4, 5, 6, 7]}
      controlMode="impedance"
    />
    
    <ParallelGripper
      name="gripper"
      interface="serial"
      device="/dev/ttyUSB1"
      maxForce={40}
    />
  </ComponentGroup>
);
```

---

## 6. Task Graph Definition

### 6.1 Basic Task Definition

```tsx
import { defineTask, Priority, Duration } from '@hefaos/sdk';

// Simple task
export const ReadIMU = defineTask({
  name: 'ReadIMU',
  
  // Timing configuration
  timing: {
    period: Duration.ms(1),      // 1kHz
    deadline: Duration.us(500),  // Must complete within 500μs
    wcet: Duration.us(100),      // Expected worst-case time
  },
  
  // Priority band
  priority: Priority.CONTROL,
  
  // Resource requirements
  resources: {
    realtimeCritical: true,
    cpuAffinity: 'rt',  // Run on RT cores
  },
  
  // Input/output components
  inputs: {},
  outputs: {
    imu: IMUReading,
  },
  
  // Implementation reference
  implementation: 'hefaos::tasks::ReadIMUTask',
});

// Task with dependencies
export const StateEstimation = defineTask({
  name: 'StateEstimation',
  
  timing: {
    period: Duration.ms(1),
    deadline: Duration.us(700),
    wcet: Duration.us(150),
  },
  
  priority: Priority.CONTROL,
  
  // Explicit dependencies
  dependsOn: [ReadIMU, ReadEncoders],
  
  inputs: {
    imu: IMUReading,
    joints: JointState.array(7),
  },
  
  outputs: {
    estimate: StateEstimate,
  },
  
  implementation: 'hefaos::tasks::EKFEstimationTask',
});
```

### 6.2 Task Graph Composition

```tsx
import { TaskGraph, Sequence, Parallel } from '@hefaos/sdk';

// Declarative task graph
export const ControlLoop = () => (
  <TaskGraph name="ControlLoop" rate="1kHz">
    {/* These run in parallel (no dependencies between them) */}
    <Parallel>
      <ReadIMU />
      <ReadEncoders />
    </Parallel>
    
    {/* This waits for both parallel tasks */}
    <StateEstimation />
    
    {/* These run in sequence after estimation */}
    <Sequence>
      <SafetyCheck />
      <ComputeControl />
      <SendMotorCommands />
    </Sequence>
  </TaskGraph>
);

// Perception pipeline (different rate)
export const PerceptionPipeline = () => (
  <TaskGraph name="PerceptionPipeline" rate="30Hz">
    <CaptureCamera />
    <RunObjectDetection model="yolov8s" />
    <UpdateWorldModel />
  </TaskGraph>
);

// Planning loop
export const PlanningLoop = () => (
  <TaskGraph name="PlanningLoop" rate="10Hz">
    <GetGoal />
    <PlanMotion />
    <ValidateTrajectory />
    <PublishTrajectory />
  </TaskGraph>
);
```

### 6.3 Advanced Task Patterns

```tsx
// Conditional task execution
export const AdaptiveControl = () => (
  <TaskGraph name="AdaptiveControl" rate="1kHz">
    <ReadSensors />
    
    <ConditionalTask
      condition={(state) => state.contactDetected}
      then={<ImpedanceControl />}
      else={<PositionControl />}
    />
    
    <SendCommands />
  </TaskGraph>
);

// Task with timeout and fallback
export const RobustPerception = () => (
  <TaskGraph name="RobustPerception" rate="30Hz">
    <CaptureCamera />
    
    <WithTimeout 
      timeout={Duration.ms(25)}
      fallback={<UseLastDetection />}
    >
      <RunObjectDetection />
    </WithTimeout>
    
    <UpdateWorldModel />
  </TaskGraph>
);

// Multi-rate synchronization
export const SynchronizedLoop = () => (
  <TaskGraph name="SynchronizedLoop">
    {/* 1kHz control loop */}
    <RateGroup rate="1kHz" priority={Priority.CONTROL}>
      <ReadIMU />
      <StateEstimation />
      <Control />
    </RateGroup>
    
    {/* 100Hz force control (synchronized to control loop) */}
    <RateGroup rate="100Hz" synchronizedTo="1kHz" divisor={10}>
      <ReadForceTorque />
      <ForceControl />
    </RateGroup>
    
    {/* 30Hz perception (asynchronous) */}
    <RateGroup rate="30Hz" priority={Priority.PERCEPTION}>
      <CaptureCamera />
      <RunDetection />
    </RateGroup>
  </TaskGraph>
);
```

### 6.4 Visual Task Graph Builder

The SDK includes a visual tool for building task graphs:

```tsx
// hefaos.config.ts
export default {
  devServer: {
    port: 3000,
    enableVisualEditor: true,
    hotReload: true,
  },
  
  visualEditor: {
    // Enable drag-and-drop task graph editing
    enabled: true,
    // Auto-generate TypeScript from visual edits
    syncToCode: true,
  },
};
```

Visual editor generates:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      Task Graph Visual Editor                            │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌──────────┐    ┌──────────┐                                          │
│   │  ReadIMU │    │ReadEncode│                                          │
│   │   1kHz   │    │   1kHz   │                                          │
│   └────┬─────┘    └────┬─────┘                                          │
│        │               │                                                 │
│        └───────┬───────┘                                                 │
│                │                                                         │
│                ▼                                                         │
│        ┌──────────────┐                                                 │
│        │ StateEstimate│                                                 │
│        │     1kHz     │                                                 │
│        └──────┬───────┘                                                 │
│               │                                                          │
│        ┌──────┴───────┐                                                 │
│        │              │                                                  │
│        ▼              ▼                                                  │
│  ┌───────────┐  ┌───────────┐                                          │
│  │SafetyCheck│  │  Control  │                                          │
│  └───────────┘  └─────┬─────┘                                          │
│                       │                                                  │
│                       ▼                                                  │
│               ┌─────────────┐                                           │
│               │ MotorSend   │                                           │
│               └─────────────┘                                           │
│                                                                          │
│   [+ Add Task]  [Connect]  [Delete]  [Properties Panel ▶]              │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 7. Robot Composition

### 7.1 Complete Robot Definition

```tsx
// robot.tsx
import { defineRobot } from '@hefaos/sdk';
import { SensorSuite, ActuatorSuite } from './components';
import { ControlLoop, PerceptionPipeline, PlanningLoop } from './tasks';
import { PickAndPlaceBehavior } from './behaviors';
import { models } from './models';

export const ManipulatorRobot = defineRobot({
  name: 'Manipulator',
  
  // Robot description
  urdf: './robot.urdf',
  
  // Mount components
  components: (
    <>
      <SensorSuite />
      <ActuatorSuite />
      <SafetyMCU port="/dev/ttyACM0" baudRate={1000000} />
    </>
  ),
  
  // Define task graphs
  tasks: (
    <>
      <ControlLoop />
      <PerceptionPipeline />
      <PlanningLoop />
    </>
  ),
  
  // Register AI models
  models: models,
  
  // Default behavior
  behavior: <PickAndPlaceBehavior />,
  
  // Safety configuration
  safety: {
    velocityLimit: 1.0,          // rad/s
    accelerationLimit: 5.0,      // rad/s²
    forceLimit: 50.0,            // N
    watchdogTimeout: Duration.ms(10),
  },
  
  // Coordinate frames
  frames: {
    base: 'base_link',
    endEffector: 'tool0',
    camera: 'camera_link',
  },
});

export default ManipulatorRobot;
```

### 7.2 Robot Variants

```tsx
// Define a base robot
const BaseManipulator = defineRobot({
  name: 'BaseManipulator',
  urdf: './robot.urdf',
  components: <BaseComponents />,
  tasks: <BaseTasks />,
});

// Create variants by extending
export const LabManipulator = extendRobot(BaseManipulator, {
  name: 'LabManipulator',
  
  // Override components
  components: (base) => (
    <>
      {base}
      <MockSafetyMCU />  {/* Use simulated safety for lab */}
    </>
  ),
  
  // Add development tools
  devtools: {
    enableVisualization: true,
    enableRemoteControl: true,
    logLevel: 'debug',
  },
});

export const ProductionManipulator = extendRobot(BaseManipulator, {
  name: 'ProductionManipulator',
  
  components: (base) => (
    <>
      {base}
      <RealSafetyMCU port="/dev/ttyACM0" />
    </>
  ),
  
  safety: {
    // Stricter limits for production
    velocityLimit: 0.5,
    forceLimit: 30.0,
  },
});
```

### 7.3 Multi-Robot Systems

```tsx
// fleet.tsx
import { defineFleet, RobotInstance } from '@hefaos/sdk';
import { ManipulatorRobot } from './robot';

export const WarehouseFleet = defineFleet({
  name: 'WarehouseFleet',
  
  robots: [
    <RobotInstance 
      robot={ManipulatorRobot}
      id="arm_1"
      namespace="/station_1"
      config={{
        safetyMcu: { port: '/dev/ttyACM0' },
        canInterface: 'can0',
      }}
    />,
    
    <RobotInstance
      robot={ManipulatorRobot}
      id="arm_2"
      namespace="/station_2"
      config={{
        safetyMcu: { port: '/dev/ttyACM1' },
        canInterface: 'can1',
      }}
    />,
  ],
  
  // Fleet-level coordination
  coordinator: <FleetCoordinator />,
  
  // Shared state
  sharedState: {
    taskQueue: TaskQueue,
    worldModel: SharedWorldModel,
  },
});
```

---

## 8. Behavior Trees

### 8.1 Behavior Tree Primitives

```tsx
import { 
  BehaviorTree, 
  Sequence, 
  Selector, 
  Parallel,
  Action,
  Condition,
  Decorator 
} from '@hefaos/behaviors';

// Simple behavior tree
export const PickAndPlaceBehavior = () => (
  <BehaviorTree name="PickAndPlace">
    <Sequence>
      {/* Check preconditions */}
      <Condition check={hasTarget} />
      <Condition check={isCalibrated} />
      
      {/* Execute pick */}
      <Sequence name="Pick">
        <Action action={moveToPreGrasp} />
        <Action action={openGripper} />
        <Action action={moveToGrasp} />
        <Action action={closeGripper} />
        <Condition check={hasObject} />
        <Action action={moveToRetract} />
      </Sequence>
      
      {/* Execute place */}
      <Sequence name="Place">
        <Action action={moveToPrePlace} />
        <Action action={moveToPlace} />
        <Action action={openGripper} />
        <Action action={moveToRetract} />
      </Sequence>
    </Sequence>
  </BehaviorTree>
);
```

### 8.2 Advanced Behavior Patterns

```tsx
// Robust behavior with fallbacks
export const RobustGrasp = () => (
  <BehaviorTree name="RobustGrasp">
    <Selector>
      {/* Try primary grasp strategy */}
      <Sequence>
        <Condition check={hasGraspPose} />
        <Action action={executeGrasp} />
      </Sequence>
      
      {/* Fallback: Re-detect and try again */}
      <Sequence>
        <Action action={moveToScanPose} />
        <Action action={detectObjects} />
        <Action action={computeGrasp} />
        <Action action={executeGrasp} />
      </Sequence>
      
      {/* Final fallback: Ask for help */}
      <Action action={requestHumanAssistance} />
    </Selector>
  </BehaviorTree>
);

// Behavior with decorators
export const TimedAction = () => (
  <BehaviorTree name="TimedAction">
    <Decorator type="timeout" duration={Duration.seconds(30)}>
      <Decorator type="retry" maxAttempts={3}>
        <Sequence>
          <Action action={riskyOperation} />
          <Action action={verifyResult} />
        </Sequence>
      </Decorator>
    </Decorator>
  </BehaviorTree>
);

// Parallel behaviors
export const MonitoredExecution = () => (
  <BehaviorTree name="MonitoredExecution">
    <Parallel policy="successOnAll" failurePolicy="failOnOne">
      {/* Main task */}
      <Sequence>
        <Action action={executeTrajectory} />
      </Sequence>
      
      {/* Safety monitor (runs in parallel) */}
      <Decorator type="repeat" until="forever">
        <Sequence>
          <Condition check={isSafe} />
          <Action action={sleep} duration={Duration.ms(10)} />
        </Sequence>
      </Decorator>
      
      {/* Force monitor */}
      <Decorator type="repeat" until="forever">
        <Selector>
          <Condition check={forceWithinLimits} />
          <Action action={emergencyStop} />
        </Selector>
      </Decorator>
    </Parallel>
  </BehaviorTree>
);
```

### 8.3 Behavior Actions (Implementation)

```tsx
// Define actions that can be used in behavior trees
import { defineAction } from '@hefaos/behaviors';

export const moveToPreGrasp = defineAction({
  name: 'MoveToPreGrasp',
  
  // Input parameters
  params: {
    target: Schema.pose(),
    velocity: Schema.number().default(0.5),
  },
  
  // Required state
  requires: {
    estimate: StateEstimate,
    worldModel: WorldModel,
  },
  
  // Implementation
  implementation: async (params, state, context) => {
    const preGraspPose = computePreGrasp(params.target);
    const trajectory = await context.planner.planTo(preGraspPose, {
      velocity: params.velocity,
    });
    
    if (!trajectory) {
      return ActionResult.FAILURE;
    }
    
    await context.executor.execute(trajectory);
    return ActionResult.SUCCESS;
  },
  
  // Or reference C++ implementation
  // implementation: 'hefaos::actions::MoveToPreGrasp',
});

export const closeGripper = defineAction({
  name: 'CloseGripper',
  
  params: {
    force: Schema.number().default(20),  // N
    width: Schema.number().optional(),   // mm
  },
  
  implementation: 'hefaos::actions::CloseGripper',
});
```

---

## 9. AI Model Integration

### 9.1 Model Definition

```tsx
// models.tsx
import { defineModel, ModelPriority } from '@hefaos/sdk';

export const objectDetectionModel = defineModel({
  name: 'ObjectDetection',
  
  // Model file
  path: './models/yolov8s.onnx',
  
  // Backend preference
  backend: ['tensorrt', 'onnx'],  // Try TensorRT first, fall back to ONNX
  
  // Scheduling priority
  priority: ModelPriority.PERCEPTION,
  
  // Timing constraints
  timing: {
    deadline: Duration.ms(30),
    // Skip inference if we're behind
    skipOnDeadlineMiss: true,
  },
  
  // Input specification
  inputs: {
    image: {
      shape: [1, 3, 640, 640],
      dtype: 'float32',
      preprocessing: 'normalize_imagenet',
    },
  },
  
  // Output specification
  outputs: {
    detections: {
      shape: [1, 84, 8400],
      dtype: 'float32',
      postprocessing: 'yolo_nms',
    },
  },
  
  // Optimization
  optimization: {
    fp16: true,
    int8: false,
    dynamicBatching: false,
  },
});

export const graspPolicyModel = defineModel({
  name: 'GraspPolicy',
  
  path: './models/grasp_policy.tflite',
  backend: 'tflite',
  priority: ModelPriority.CONTROL,
  
  timing: {
    deadline: Duration.us(800),
    skipOnDeadlineMiss: false,  // Control model must run
  },
  
  inputs: {
    state: {
      shape: [1, 14],  // 7 positions + 7 velocities
      dtype: 'float32',
    },
    target: {
      shape: [1, 6],   // target pose
      dtype: 'float32',
    },
  },
  
  outputs: {
    torque: {
      shape: [1, 7],
      dtype: 'float32',
    },
  },
});

export const llmAgent = defineModel({
  name: 'LLMAgent',
  
  path: './models/llama3-8b-q4.gguf',
  backend: 'llama_cpp',
  priority: ModelPriority.REASONING,
  
  timing: {
    deadline: Duration.seconds(5),
    skipOnDeadlineMiss: true,
  },
  
  config: {
    contextLength: 4096,
    temperature: 0.7,
    topP: 0.9,
  },
});

// Export all models
export const models = {
  objectDetection: objectDetectionModel,
  graspPolicy: graspPolicyModel,
  llmAgent: llmAgent,
};
```

### 9.2 Using Models in Tasks

```tsx
// Task that uses a model
export const ObjectDetectionTask = defineTask({
  name: 'ObjectDetection',
  
  timing: {
    period: Duration.ms(33),
    deadline: Duration.ms(30),
  },
  
  priority: Priority.PERCEPTION,
  
  inputs: {
    camera: CameraFrame,
  },
  
  outputs: {
    detections: Detections,
  },
  
  // Reference the model
  model: objectDetectionModel,
  
  // Task implementation uses the model
  implementation: async (inputs, model, context) => {
    const tensor = preprocessImage(inputs.camera);
    const output = await model.infer({ image: tensor });
    const detections = postprocessDetections(output.detections);
    return { detections };
  },
});
```

### 9.3 Model Chaining

```tsx
// Chain multiple models
export const GraspPipeline = () => (
  <ModelPipeline name="GraspPipeline">
    {/* Stage 1: Detect objects */}
    <ModelStage 
      model={objectDetectionModel}
      input={(frame) => ({ image: preprocessImage(frame) })}
      output="detections"
    />
    
    {/* Stage 2: Estimate poses */}
    <ModelStage
      model={poseEstimationModel}
      input={(_, detections) => ({ 
        crops: extractCrops(detections) 
      })}
      output="poses"
    />
    
    {/* Stage 3: Compute grasp */}
    <ModelStage
      model={graspPolicyModel}
      input={(_, __, poses) => ({
        objectPoses: poses,
        robotState: getCurrentState(),
      })}
      output="graspPose"
    />
  </ModelPipeline>
);
```

---

## 10. State Management

### 10.1 Reactive State (Signals)

```tsx
import { createSignal, createMemo, createEffect } from '@hefaos/state';

// Create reactive signals
const [imuReading, setIMU] = createSignal<IMUReading>();
const [jointState, setJoints] = createSignal<JointState[]>();
const [targetPose, setTarget] = createSignal<Pose>();

// Derived state (automatically recomputes when dependencies change)
const stateEstimate = createMemo(() => {
  const imu = imuReading();
  const joints = jointState();
  if (!imu || !joints) return null;
  
  return kalmanFilter.update(imu, joints);
});

const controlCommand = createMemo(() => {
  const estimate = stateEstimate();
  const target = targetPose();
  if (!estimate || !target) return null;
  
  return impedanceController.compute(estimate, target);
});

// Side effects
createEffect(() => {
  const cmd = controlCommand();
  if (cmd) {
    motorDriver.send(cmd);
    logger.log('command', cmd);
  }
});
```

### 10.2 State Store

```tsx
import { createStore, produce } from '@hefaos/state';

// Define store shape
interface RobotStore {
  sensors: {
    imu: IMUReading | null;
    joints: JointState[];
    camera: CameraFrame | null;
  };
  state: {
    estimate: StateEstimate | null;
    mode: 'idle' | 'running' | 'error';
  };
  world: {
    objects: Detection[];
    obstacles: Obstacle[];
  };
}

// Create store
const robotStore = createStore<RobotStore>({
  sensors: {
    imu: null,
    joints: [],
    camera: null,
  },
  state: {
    estimate: null,
    mode: 'idle',
  },
  world: {
    objects: [],
    obstacles: [],
  },
});

// Update store (immutable updates with immer-style produce)
robotStore.update(produce(draft => {
  draft.sensors.imu = newIMUReading;
}));

// Subscribe to changes
robotStore.subscribe(
  state => state.state.estimate,
  estimate => {
    // Called when estimate changes
    console.log('New estimate:', estimate);
  }
);

// Select derived state
const robotPose = robotStore.select(state => state.state.estimate?.pose);
```

### 10.3 Cross-Process State (via IPC)

```tsx
import { createSharedState } from '@hefaos/state';

// State shared across processes via iceoryx
const sharedWorldModel = createSharedState({
  topic: '/world_model',
  schema: WorldModel,
  
  // Initial value
  initial: {
    objects: [],
    robotPose: null,
    timestamp: 0,
  },
  
  // Update policy
  updatePolicy: 'latest',  // or 'merge', 'queue'
});

// In perception process
sharedWorldModel.publish({
  objects: detectedObjects,
  robotPose: currentPose,
  timestamp: Date.now(),
});

// In planning process
const worldModel = sharedWorldModel.subscribe();
createEffect(() => {
  const world = worldModel();
  // React to world model updates
  replanIfNeeded(world);
});
```

---

## 11. Hooks and Lifecycle

### 11.1 Component Lifecycle Hooks

```tsx
import { 
  onMount, 
  onUnmount, 
  onUpdate,
  onError,
  useRobotState,
  useModel,
  useTask 
} from '@hefaos/hooks';

export const PerceptionComponent = () => {
  // Access robot state
  const { camera, worldModel } = useRobotState();
  
  // Access a model
  const detector = useModel('objectDetection');
  
  // Lifecycle hooks
  onMount(() => {
    console.log('Perception component mounted');
    detector.warmup();
  });
  
  onUnmount(() => {
    console.log('Perception component unmounting');
  });
  
  onError((error) => {
    console.error('Perception error:', error);
    // Graceful degradation
    useLastKnownDetections();
  });
  
  return (
    <TaskGraph name="Perception">
      <CaptureCamera />
      <DetectObjects model={detector} />
      <UpdateWorldModel />
    </TaskGraph>
  );
};
```

### 11.2 Custom Hooks

```tsx
// Create reusable hooks
export function useGrasp() {
  const gripper = useActuator('gripper');
  const forceSensor = useSensor('wrist_ft');
  const [isGrasping, setGrasping] = createSignal(false);
  
  const grasp = async (force: number) => {
    setGrasping(true);
    await gripper.close(force);
    
    // Wait for stable grasp
    await waitUntil(() => {
      const ft = forceSensor.read();
      return ft.force.z > force * 0.8;
    }, { timeout: Duration.seconds(2) });
    
    setGrasping(false);
    return gripper.hasObject();
  };
  
  const release = async () => {
    setGrasping(true);
    await gripper.open();
    setGrasping(false);
  };
  
  return { grasp, release, isGrasping };
}

// Use in behaviors
export const GraspBehavior = () => {
  const { grasp, release, isGrasping } = useGrasp();
  const targetPose = useRobotState(s => s.target);
  
  return (
    <BehaviorTree name="Grasp">
      <Sequence>
        <MoveTo pose={targetPose} />
        <Action action={() => grasp(20)} />
        <Condition check={hasObject} />
      </Sequence>
    </BehaviorTree>
  );
};
```

### 11.3 Timing Hooks

```tsx
import { useInterval, useTimeout, useDeadline } from '@hefaos/hooks';

export const PeriodicTask = () => {
  // Run at fixed interval
  useInterval(() => {
    checkSystemHealth();
  }, Duration.seconds(1));
  
  // One-shot timeout
  useTimeout(() => {
    console.log('Warmup complete');
  }, Duration.seconds(5));
  
  // Deadline-aware execution
  const result = useDeadline(
    async () => {
      return await computeExpensiveResult();
    },
    Duration.ms(100),
    // Fallback if deadline missed
    () => getLastResult()
  );
  
  return <TaskGraph>...</TaskGraph>;
};
```

---

## 12. Type System

### 12.1 Schema Types

```tsx
import { Schema, infer } from '@hefaos/schema';

// Define schemas
const Vec3Schema = Schema.object({
  x: Schema.float64(),
  y: Schema.float64(),
  z: Schema.float64(),
});

const PoseSchema = Schema.object({
  position: Vec3Schema,
  orientation: Schema.object({
    w: Schema.float64(),
    x: Schema.float64(),
    y: Schema.float64(),
    z: Schema.float64(),
  }),
});

// Infer TypeScript types from schemas
type Vec3 = infer<typeof Vec3Schema>;
// { x: number; y: number; z: number }

type Pose = infer<typeof PoseSchema>;
// { position: Vec3; orientation: { w: number; x: number; y: number; z: number } }

// Schema with constraints
const JointStateSchema = Schema.object({
  position: Schema.float64().min(-Math.PI).max(Math.PI),
  velocity: Schema.float64().min(-10).max(10),
  effort: Schema.float64().min(-100).max(100),
});

// Array schemas
const JointStatesSchema = Schema.array(JointStateSchema).length(7);

// Optional and nullable
const DetectionSchema = Schema.object({
  classId: Schema.uint32(),
  confidence: Schema.float32().min(0).max(1),
  bbox: BoundingBoxSchema,
  pose3d: Schema.optional(PoseSchema),
});
```

### 12.2 Component Type Safety

```tsx
// Type-safe component props
interface IMUSensorProps {
  device: string;
  sampleRate: 100 | 200 | 400 | 800 | 1000;
  accelRange: '2g' | '4g' | '8g' | '16g';
  gyroRange: '250dps' | '500dps' | '1000dps' | '2000dps';
}

const IMUSensor: Component<IMUSensorProps> = (props) => {
  // TypeScript ensures props are correct
  return <HardwareSensor driver="BMI088" config={props} />;
};

// Usage - TypeScript catches errors
<IMUSensor 
  device="/dev/spi0"
  sampleRate={1000}        // ✓ Valid
  accelRange="4g"          // ✓ Valid
  gyroRange="invalid"      // ✗ Error: Type '"invalid"' is not assignable
/>
```

### 12.3 Task Type Safety

```tsx
// Type-safe task definitions
const StateEstimation = defineTask({
  name: 'StateEstimation',
  
  inputs: {
    imu: IMUReadingSchema,
    joints: JointStatesSchema,
  },
  
  outputs: {
    estimate: StateEstimateSchema,
  },
  
  implementation: (inputs, context) => {
    // TypeScript infers types from schemas
    const { imu, joints } = inputs;
    // imu: IMUReading, joints: JointState[]
    
    const estimate = computeEstimate(imu, joints);
    return { estimate };  // Must match output schema
  },
});
```

---

## 13. Code Generation

### 13.1 Generated Artifacts

The HefaOS compiler transforms TypeScript/JSX into:

```
TypeScript Source                    Generated Artifacts
─────────────────                    ───────────────────

robot.tsx ──────────────────────────► generated/
                                      ├── cpp/
components/*.tsx ───────────────────► │   ├── components.hpp
                                      │   ├── components.cpp
tasks.tsx ──────────────────────────► │   ├── task_graph.hpp
                                      │   └── task_graph.cpp
                                      │
behaviors/*.tsx ────────────────────► ├── behaviors/
                                      │   ├── pick_and_place.xml
                                      │   └── ...
                                      │
models.tsx ─────────────────────────► ├── models/
                                      │   ├── manifest.yaml
                                      │   └── configs/
                                      │
schemas (inferred) ─────────────────► ├── schemas/
                                      │   ├── imu_reading.fbs
                                      │   ├── joint_state.fbs
                                      │   └── ...
                                      │
                                      ├── config/
                                      │   ├── robot.yaml
                                      │   ├── tasks.yaml
                                      │   └── safety.yaml
                                      │
                                      └── types/
                                          └── index.d.ts
```

### 13.2 Generated C++ Example

```cpp
// generated/cpp/components.hpp
#pragma once

#include <hefaos/core.hpp>
#include <hefaos/components.hpp>

namespace generated {

// Component registration (from TypeScript definitions)
inline void register_components(hefaos::ComponentRegistry& registry) {
    // IMUSensor component
    registry.register_component<hefaos::IMUReading>(
        "IMUSensor",
        hefaos::ComponentConfig{
            .driver = "hefaos::drivers::BMI088Driver",
            .hardware = hefaos::HardwareConfig{
                .type = hefaos::HardwareType::SPI,
                .device = "/dev/spidev0.0",
            },
            .params = {
                {"sample_rate", 1000},
                {"accel_range", "4g"},
                {"gyro_range", "500dps"},
            },
        }
    );
    
    // Additional components...
}

}  // namespace generated
```

```cpp
// generated/cpp/task_graph.hpp
#pragma once

#include <hefaos/task_graph.hpp>

namespace generated {

inline hefaos::TaskGraph create_control_loop() {
    hefaos::TaskGraph graph("ControlLoop");
    
    // Task definitions (from TypeScript)
    auto read_imu = graph.add_task({
        .name = "ReadIMU",
        .period = std::chrono::milliseconds(1),
        .deadline = std::chrono::microseconds(500),
        .priority = hefaos::Priority::CONTROL,
        .implementation = "hefaos::tasks::ReadIMUTask",
    });
    
    auto read_encoders = graph.add_task({
        .name = "ReadEncoders",
        .period = std::chrono::milliseconds(1),
        .deadline = std::chrono::microseconds(500),
        .priority = hefaos::Priority::CONTROL,
        .implementation = "hefaos::tasks::ReadEncodersTask",
    });
    
    auto state_estimation = graph.add_task({
        .name = "StateEstimation",
        .period = std::chrono::milliseconds(1),
        .deadline = std::chrono::microseconds(700),
        .priority = hefaos::Priority::CONTROL,
        .implementation = "hefaos::tasks::EKFEstimationTask",
    });
    
    // Dependencies (from JSX structure)
    graph.add_dependency(read_imu, state_estimation);
    graph.add_dependency(read_encoders, state_estimation);
    
    // ... more tasks
    
    return graph;
}

}  // namespace generated
```

### 13.3 Generated FlatBuffer Schema

```flatbuffers
// generated/schemas/imu_reading.fbs
// Auto-generated from TypeScript component definition

namespace hefaos.msgs;

struct Vec3 {
    x: float64;
    y: float64;
    z: float64;
}

table IMUReading {
    linear_acceleration: Vec3 (id: 0);
    angular_velocity: Vec3 (id: 1);
    timestamp: uint64 (id: 2);
    sequence: uint32 (id: 3);
}

root_type IMUReading;
```

### 13.4 Generated Configuration

```yaml
# generated/config/robot.yaml
# Auto-generated from robot.tsx

name: Manipulator
urdf: robot.urdf

components:
  sensors:
    imu:
      driver: hefaos::drivers::BMI088Driver
      hardware:
        type: spi
        device: /dev/spidev0.0
      config:
        sample_rate: 1000
        accel_range: 4g
        gyro_range: 500dps
        
    encoders:
      driver: hefaos::drivers::AS5048ADriver
      hardware:
        type: spi
        device: /dev/spidev0.1
      config:
        joint_count: 7
        
  actuators:
    motors:
      driver: hefaos::drivers::CyberGearDriver
      hardware:
        type: can
        interface: can0
      config:
        motor_ids: [1, 2, 3, 4, 5, 6, 7]
        control_mode: impedance

task_graphs:
  - name: ControlLoop
    config: graphs/control_loop.yaml
  - name: PerceptionPipeline
    config: graphs/perception.yaml
  - name: PlanningLoop
    config: graphs/planning.yaml

models:
  object_detection:
    path: models/yolov8s.onnx
    config: models/configs/object_detection.yaml
  grasp_policy:
    path: models/grasp_policy.tflite
    config: models/configs/grasp_policy.yaml

safety:
  velocity_limit: 1.0
  acceleration_limit: 5.0
  force_limit: 50.0
  watchdog_timeout_ms: 10
```

---

## 14. Development Workflow

### 14.1 Project Structure

```
my-robot/
├── package.json
├── tsconfig.json
├── hefaos.config.ts
│
├── src/
│   ├── robot.tsx           # Main robot definition
│   ├── components/
│   │   ├── sensors.tsx
│   │   └── actuators.tsx
│   ├── tasks/
│   │   ├── control.tsx
│   │   ├── perception.tsx
│   │   └── planning.tsx
│   ├── behaviors/
│   │   ├── pick-and-place.tsx
│   │   └── homing.tsx
│   ├── models/
│   │   └── index.tsx
│   └── state/
│       └── store.tsx
│
├── models/                  # AI model files
│   ├── yolov8s.onnx
│   └── grasp_policy.tflite
│
├── urdf/                    # Robot description
│   └── robot.urdf
│
├── generated/               # Compiler output (git-ignored)
│   ├── cpp/
│   ├── schemas/
│   ├── config/
│   └── types/
│
└── tests/
    ├── unit/
    └── integration/
```

### 14.2 Development Commands

```bash
# Initialize new project
npx create-hefaos-app my-robot

# Development mode (hot reload in simulation)
npm run dev

# Type checking
npm run typecheck

# Build (generate artifacts)
npm run build

# Build and deploy to robot
npm run deploy --target robot@192.168.1.100

# Run tests
npm run test

# Run in simulation
npm run sim

# Launch visual editor
npm run editor
```

### 14.3 hefaos.config.ts

```tsx
import { defineConfig } from '@hefaos/sdk';

export default defineConfig({
  // Project info
  name: 'my-robot',
  version: '1.0.0',
  
  // Entry point
  entry: './src/robot.tsx',
  
  // Output directory
  outDir: './generated',
  
  // Target hardware
  target: {
    platform: 'linux',
    arch: 'arm64',
    board: 'jetson_orin',
  },
  
  // Code generation options
  codegen: {
    // Generate C++ code
    cpp: {
      enabled: true,
      standard: 'c++20',
      outputDir: './generated/cpp',
    },
    
    // Generate FlatBuffer schemas
    flatbuffers: {
      enabled: true,
      outputDir: './generated/schemas',
    },
    
    // Generate TypeScript types for development
    typescript: {
      enabled: true,
      outputDir: './generated/types',
    },
  },
  
  // Development server
  devServer: {
    port: 3000,
    enableHotReload: true,
    enableVisualEditor: true,
    
    // Simulation backend
    simulation: {
      enabled: true,
      backend: 'mujoco',
      modelPath: './simulation/robot.xml',
    },
  },
  
  // Compiler plugins
  plugins: [
    '@hefaos/plugin-behavior-tree',
    '@hefaos/plugin-motion-planning',
  ],
});
```

### 14.4 Hot Reload Workflow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      Development Workflow                                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   Developer                         HefaOS Dev Server                     │
│   ──────────                        ─────────────────                    │
│                                                                          │
│   Edit robot.tsx ────────────────► File Watcher                         │
│                                          │                               │
│                                          ▼                               │
│                                    Parse & Validate                      │
│                                          │                               │
│                    ┌─────────────────────┼─────────────────────┐        │
│                    │                     │                     │        │
│                    ▼                     ▼                     ▼        │
│              Update Configs       Update Behaviors      Update Models   │
│                    │                     │                     │        │
│                    └─────────────────────┼─────────────────────┘        │
│                                          │                               │
│                                          ▼                               │
│                                   Simulation Runtime                     │
│                                          │                               │
│   Visual Feedback ◄──────────────────────┘                              │
│   (Visualizer)                                                          │
│                                                                          │
│   ┌──────────────────────────────────────────────────────────────────┐  │
│   │                                                                   │  │
│   │   Changes reflected in ~100ms (behavior/config changes)           │  │
│   │   Full rebuild for structural changes (new components)            │  │
│   │                                                                   │  │
│   └──────────────────────────────────────────────────────────────────┘  │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 15. Tooling

### 15.1 VS Code Extension

```json
// .vscode/extensions.json
{
  "recommendations": [
    "hefaos.hefaos-vscode",
    "dbaeumer.vscode-eslint",
    "esbenp.prettier-vscode"
  ]
}
```

Features:
- Syntax highlighting for HefaOS-specific JSX
- IntelliSense for component props and schemas
- Go-to-definition for C++ implementations
- Inline task timing visualization
- Behavior tree visual preview
- Real-time type checking

### 15.2 CLI Tools

```bash
# HefaOS CLI
hefaos --help

Commands:
  hefaos init          Initialize a new HefaOS project
  hefaos build         Build the project (generate artifacts)
  hefaos dev           Start development server
  hefaos deploy        Deploy to target robot
  hefaos sim           Run in simulation
  hefaos test          Run tests
  hefaos lint          Lint TypeScript source
  hefaos doctor        Check project health
  hefaos upgrade       Upgrade HefaOS SDK version

# Examples
hefaos build --target arm64 --release
hefaos deploy --host robot@192.168.1.100 --sync-models
hefaos sim --backend mujoco --visualize
```

### 15.3 Visual Tools

```tsx
// Launch visual tools programmatically
import { launchVisualizer, launchEditor, launchProfiler } from '@hefaos/devtools';

// 3D robot visualizer
launchVisualizer({
  urdf: './robot.urdf',
  stateSource: robotStore,
});

// Task graph editor
launchEditor({
  taskGraphs: ['ControlLoop', 'PerceptionPipeline'],
  syncToCode: true,
});

// Performance profiler
launchProfiler({
  tasks: ['ReadIMU', 'StateEstimation', 'Control'],
  showTimeline: true,
  showHistograms: true,
});
```

### 15.4 Testing Utilities

```tsx
import { createTestRobot, mockSensor, mockActuator, advanceTime } from '@hefaos/testing';

describe('ControlLoop', () => {
  it('should compute control commands at 1kHz', async () => {
    // Create test robot with mocked hardware
    const robot = createTestRobot(ManipulatorRobot, {
      mocks: {
        imu: mockSensor('IMU', {
          read: () => ({
            linearAcceleration: { x: 0, y: 0, z: 9.81 },
            angularVelocity: { x: 0, y: 0, z: 0 },
            timestamp: Date.now(),
          }),
        }),
        motors: mockActuator('Motors', {
          send: jest.fn(),
        }),
      },
    });
    
    // Run for 100ms
    await advanceTime(Duration.ms(100));
    
    // Verify control loop ran ~100 times
    expect(robot.mocks.motors.send).toHaveBeenCalledTimes(100);
  });
});
```

---

## 16. Examples

### 16.1 Complete Pick and Place Robot

```tsx
// src/robot.tsx
import { defineRobot } from '@hefaos/sdk';
import { BMI088, AS5048A, RealSenseD435, CyberGearMotor, ParallelGripper } from '@hefaos/components';
import { ControlLoop, PerceptionPipeline, PlanningLoop } from './tasks';
import { PickAndPlaceBehavior } from './behaviors';
import { models } from './models';

export const PickAndPlaceRobot = defineRobot({
  name: 'PickAndPlaceRobot',
  urdf: './urdf/robot.urdf',
  
  components: (
    <>
      {/* Sensors */}
      <BMI088 
        name="imu"
        device="/dev/spidev0.0"
        sampleRate={1000}
        accelRange="4g"
        gyroRange="500dps"
      />
      
      <AS5048A
        name="encoders"
        device="/dev/spidev0.1"
        joints={7}
      />
      
      <RealSenseD435
        name="camera"
        serial="12345678"
        resolution={[640, 480]}
        fps={30}
        enableDepth={true}
      />
      
      {/* Actuators */}
      <CyberGearMotor
        name="arm"
        canInterface="can0"
        motorIds={[1, 2, 3, 4, 5, 6, 7]}
        controlMode="impedance"
      />
      
      <ParallelGripper
        name="gripper"
        device="/dev/ttyUSB0"
        maxWidth={85}
        maxForce={40}
      />
      
      {/* Safety */}
      <SafetyMCU
        name="safety"
        port="/dev/ttyACM0"
        baudRate={1000000}
      />
    </>
  ),
  
  tasks: (
    <>
      <ControlLoop />
      <PerceptionPipeline />
      <PlanningLoop />
    </>
  ),
  
  models: models,
  
  behavior: <PickAndPlaceBehavior />,
  
  safety: {
    velocityLimit: 1.0,
    forceLimit: 50.0,
    watchdogTimeout: Duration.ms(10),
  },
});

export default PickAndPlaceRobot;
```

```tsx
// src/tasks/control.tsx
import { TaskGraph, Sequence, Parallel } from '@hefaos/sdk';

export const ControlLoop = () => (
  <TaskGraph name="ControlLoop" rate="1kHz">
    <Parallel name="ReadSensors">
      <ReadIMU deadline="500us" />
      <ReadEncoders deadline="500us" />
      <ReadForceTorque deadline="500us" />
    </Parallel>
    
    <StateEstimation deadline="200us" />
    
    <SafetyCheck deadline="50us" />
    
    <ImpedanceControl deadline="100us" />
    
    <SendMotorCommands deadline="100us" />
  </TaskGraph>
);
```

```tsx
// src/behaviors/pick-and-place.tsx
import { BehaviorTree, Sequence, Selector, Action, Condition } from '@hefaos/behaviors';
import { moveToPreGrasp, executeGrasp, moveTo, releaseObject } from '../actions';

export const PickAndPlaceBehavior = () => (
  <BehaviorTree name="PickAndPlace">
    <Selector>
      {/* Main pick and place sequence */}
      <Sequence>
        {/* Ensure we have a target */}
        <Condition check={hasTarget} />
        
        {/* Pick sequence */}
        <Sequence name="Pick">
          <Action action={moveToPreGrasp} />
          <Action action={openGripper} />
          <Action action={moveToGrasp} />
          <Action action={executeGrasp} force={20} />
          <Condition check={hasObject} />
          <Action action={liftObject} height={0.1} />
        </Sequence>
        
        {/* Place sequence */}
        <Sequence name="Place">
          <Action action={moveToPrePlace} />
          <Action action={moveToPlace} />
          <Action action={releaseObject} />
          <Action action={moveToRetract} />
        </Sequence>
        
        {/* Mark complete */}
        <Action action={markTargetComplete} />
      </Sequence>
      
      {/* Fallback: Return to home if no target */}
      <Action action={moveToHome} />
    </Selector>
  </BehaviorTree>
);
```

### 16.2 Mobile Manipulator with LLM

```tsx
// src/robot.tsx
import { defineRobot } from '@hefaos/sdk';
import { LLMAgent } from './behaviors/llm-agent';

export const MobileManipulator = defineRobot({
  name: 'MobileManipulator',
  urdf: './urdf/mobile_manipulator.urdf',
  
  components: (
    <>
      {/* Mobile base */}
      <DifferentialDrive
        leftMotor={{ canId: 1 }}
        rightMotor={{ canId: 2 }}
        wheelRadius={0.1}
        trackWidth={0.5}
      />
      
      {/* Arm */}
      <ArmComponents />
      
      {/* Sensors */}
      <VelodyneLiDAR
        name="lidar"
        device="192.168.1.201"
        rpm={600}
      />
      
      <IntelRealSense name="camera" />
    </>
  ),
  
  tasks: (
    <>
      <BaseControlLoop />
      <ArmControlLoop />
      <NavigationPipeline />
      <ManipulationPipeline />
    </>
  ),
  
  models: {
    navigation: model('nav_policy.onnx'),
    manipulation: model('manip_policy.tflite'),
    llm: model('llama3-8b.gguf', { backend: 'llama_cpp' }),
  },
  
  // LLM-powered high-level behavior
  behavior: <LLMAgent />,
});
```

```tsx
// src/behaviors/llm-agent.tsx
import { BehaviorTree, Action, Selector } from '@hefaos/behaviors';
import { useModel, useRobotState } from '@hefaos/hooks';

export const LLMAgent = () => {
  const llm = useModel('llm');
  const worldState = useRobotState(s => s.world);
  
  const processCommand = defineAction({
    name: 'ProcessCommand',
    implementation: async (params, state, context) => {
      // Get natural language command from user
      const command = await context.getCommand();
      
      // Use LLM to generate plan
      const prompt = `
        Current world state: ${JSON.stringify(worldState)}
        User command: "${command}"
        
        Generate a sequence of actions from: [navigate_to, pick_up, place_at, open_door, ...]
        Format: JSON array of {action, params}
      `;
      
      const plan = await llm.generate(prompt);
      
      // Execute the plan
      for (const step of plan.actions) {
        await context.execute(step.action, step.params);
      }
      
      return ActionResult.SUCCESS;
    },
  });
  
  return (
    <BehaviorTree name="LLMAgent">
      <Selector>
        {/* Check for new commands */}
        <Sequence>
          <Condition check={hasNewCommand} />
          <Action action={processCommand} />
        </Sequence>
        
        {/* Idle behavior */}
        <Action action={waitForCommand} />
      </Selector>
    </BehaviorTree>
  );
};
```

---

## Appendix A: Schema Reference

### A.1 Primitive Types

| Type | TypeScript | C++ | FlatBuffer |
|------|------------|-----|------------|
| `Schema.bool()` | `boolean` | `bool` | `bool` |
| `Schema.int8()` | `number` | `int8_t` | `int8` |
| `Schema.int16()` | `number` | `int16_t` | `int16` |
| `Schema.int32()` | `number` | `int32_t` | `int32` |
| `Schema.int64()` | `bigint` | `int64_t` | `int64` |
| `Schema.uint8()` | `number` | `uint8_t` | `uint8` |
| `Schema.uint16()` | `number` | `uint16_t` | `uint16` |
| `Schema.uint32()` | `number` | `uint32_t` | `uint32` |
| `Schema.uint64()` | `bigint` | `uint64_t` | `uint64` |
| `Schema.float32()` | `number` | `float` | `float` |
| `Schema.float64()` | `number` | `double` | `double` |
| `Schema.string()` | `string` | `std::string` | `string` |
| `Schema.binary()` | `Uint8Array` | `std::vector<uint8_t>` | `[uint8]` |

### A.2 Robotics Types

| Type | Description |
|------|-------------|
| `Schema.vec3()` | 3D vector (x, y, z) |
| `Schema.quat()` | Quaternion (w, x, y, z) |
| `Schema.pose()` | Position + orientation |
| `Schema.twist()` | Linear + angular velocity |
| `Schema.wrench()` | Force + torque |
| `Schema.transform()` | Homogeneous transform |
| `Schema.trajectory()` | Array of timestamped poses |

---

## Appendix B: Migration Guide from ROS2

### B.1 Concept Mapping

| ROS2 Concept | HefaOS SDK Equivalent |
|--------------|---------------------|
| Node | Component / TaskGraph |
| Topic | Shared State / IPC Channel |
| Service | Action with response |
| Action | Behavior Tree Action |
| Message | Schema-defined Component |
| Parameter | Config in component definition |
| Launch file | Robot definition (JSX) |
| URDF | Same (URDF supported) |

### B.2 Code Translation Examples

```python
# ROS2 Python
import rclpy
from sensor_msgs.msg import Imu

class IMUNode(Node):
    def __init__(self):
        super().__init__('imu_node')
        self.publisher = self.create_publisher(Imu, 'imu', 10)
        self.timer = self.create_timer(0.001, self.timer_callback)
        
    def timer_callback(self):
        msg = Imu()
        msg.linear_acceleration.x = self.read_accel_x()
        # ...
        self.publisher.publish(msg)
```

```tsx
// HefaOS SDK (TypeScript)
const ReadIMU = defineTask({
  name: 'ReadIMU',
  timing: { period: Duration.ms(1) },
  outputs: { imu: IMUReading },
  implementation: (_, context) => {
    const reading = context.hardware.imu.read();
    return { imu: reading };
  },
});
```

---

## Document Control

**Classification:** Public  
**Distribution:** Developer Community  
**Review Cycle:** Quarterly  

---

*End of SDK Specification*
