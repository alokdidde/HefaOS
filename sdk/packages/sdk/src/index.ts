/**
 * @hefaos/sdk - Hefaos Software Development Kit
 *
 * React-inspired component framework for defining robots, sensors, actuators,
 * tasks, and behaviors in TypeScript.
 *
 * @packageDocumentation
 */

// ─────────────────────────────────────────────────────────────────────────────
// Core API
// ─────────────────────────────────────────────────────────────────────────────

export { defineRobot } from './robot';
export { defineComponent } from './component';
export { defineTask } from './task';
export { defineBehavior } from './behavior';
export { defineSchema, Schema } from './schema';

// ─────────────────────────────────────────────────────────────────────────────
// Built-in Components
// ─────────────────────────────────────────────────────────────────────────────

export {
  // Sensors
  IMU,
  Camera,
  Lidar,
  ForceTorqueSensor,
  JointEncoder,

  // Actuators
  Joint,
  Gripper,
  Motor,

  // Composite
  Arm,
  MobileBase,
  Leg,
} from './components';

// ─────────────────────────────────────────────────────────────────────────────
// Task System
// ─────────────────────────────────────────────────────────────────────────────

export { Task, TaskGraph, TaskPriority } from './tasks';

// ─────────────────────────────────────────────────────────────────────────────
// Behavior System
// ─────────────────────────────────────────────────────────────────────────────

export {
  Behavior,
  BehaviorTree,
  StateMachine,
  Sequence,
  Selector,
  Parallel,
  Condition,
  Action,
} from './behaviors';

// ─────────────────────────────────────────────────────────────────────────────
// Utilities
// ─────────────────────────────────────────────────────────────────────────────

export { Duration } from './utils/duration';
export { Vec3, Quaternion, Transform } from './utils/math';

// ─────────────────────────────────────────────────────────────────────────────
// Types (re-export from @hefaos/types)
// ─────────────────────────────────────────────────────────────────────────────

export type {
  RobotDefinition,
  ComponentDefinition,
  TaskDefinition,
  BehaviorDefinition,
  SchemaDefinition,
} from '@hefaos/types';
