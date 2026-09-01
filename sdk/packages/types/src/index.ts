/**
 * @hefaos/types - Shared TypeScript type definitions
 *
 * @packageDocumentation
 */

// ─────────────────────────────────────────────────────────────────────────────
// Core Types
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Robot definition structure
 */
export interface RobotDefinition {
  name: string;
  description?: string;
  components: ComponentDefinition[];
  tasks: TaskDefinition[];
  defaultBehavior?: string;
}

/**
 * Component definition (sensors, actuators, etc.)
 */
export interface ComponentDefinition {
  type: string;
  name: string;
  props: Record<string, unknown>;
  children?: ComponentDefinition[];
}

/**
 * Task definition for the task graph
 */
export interface TaskDefinition {
  name: string;
  priority: TaskPriority;
  period: Duration;
  deadline?: Duration;
  inputs?: string[];
  outputs?: string[];
}

/**
 * Behavior definition (behavior tree or state machine)
 */
export interface BehaviorDefinition {
  name: string;
  description?: string;
  parameters?: Record<string, string>;
  tree: BehaviorNode;
}

/**
 * Schema definition for data types
 */
export interface SchemaDefinition {
  name: string;
  fields: Record<string, SchemaField>;
}

// ─────────────────────────────────────────────────────────────────────────────
// Supporting Types
// ─────────────────────────────────────────────────────────────────────────────

export type TaskPriority = 'critical' | 'high' | 'normal' | 'low';

export interface Duration {
  nanoseconds: bigint;
}

export interface BehaviorNode {
  type: string;
  name?: string;
  children?: BehaviorNode[];
  props?: Record<string, unknown>;
}

export interface SchemaField {
  type: SchemaFieldType;
  optional?: boolean;
}

export type SchemaFieldType =
  | 'int8'
  | 'int16'
  | 'int32'
  | 'int64'
  | 'uint8'
  | 'uint16'
  | 'uint32'
  | 'uint64'
  | 'float32'
  | 'float64'
  | 'bool'
  | 'string'
  | 'vec3'
  | 'quat'
  | { array: SchemaFieldType }
  | { struct: string };

// ─────────────────────────────────────────────────────────────────────────────
// Geometry Types
// ─────────────────────────────────────────────────────────────────────────────

export interface Vec3 {
  x: number;
  y: number;
  z: number;
}

export interface Quaternion {
  x: number;
  y: number;
  z: number;
  w: number;
}

export interface Transform {
  position: Vec3;
  orientation: Quaternion;
}

export interface Pose {
  position: Vec3;
  orientation: Quaternion;
  frame?: string;
}

// ─────────────────────────────────────────────────────────────────────────────
// Joint Types
// ─────────────────────────────────────────────────────────────────────────────

export type JointType = 'revolute' | 'prismatic' | 'continuous' | 'fixed';

export interface JointLimits {
  lower: number;
  upper: number;
}

export interface JointState {
  position: number;
  velocity: number;
  effort: number;
}

// ─────────────────────────────────────────────────────────────────────────────
// Sensor Types
// ─────────────────────────────────────────────────────────────────────────────

export interface IMUReading {
  linearAcceleration: Vec3;
  angularVelocity: Vec3;
  orientation?: Quaternion;
  timestamp: bigint;
}

export interface ForceTorqueReading {
  force: Vec3;
  torque: Vec3;
  timestamp: bigint;
}

// ─────────────────────────────────────────────────────────────────────────────
// Compiler Output Types
// ─────────────────────────────────────────────────────────────────────────────

export interface CompilationResult {
  cpp?: string;
  yaml?: string;
  flatbuffer?: string;
  errors: CompilationError[];
  warnings: CompilationWarning[];
}

export interface CompilationError {
  message: string;
  file?: string;
  line?: number;
  column?: number;
}

export interface CompilationWarning {
  message: string;
  file?: string;
  line?: number;
  column?: number;
}
