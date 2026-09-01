/**
 * Simple 6-DOF Robotic Arm Example
 *
 * This example demonstrates how to define a robotic arm using the Hefaos SDK.
 * The robot includes:
 * - 6 revolute joints
 * - IMU sensor
 * - Force/torque sensor at the end effector
 * - Control and monitoring tasks
 */

import {
  defineRobot,
  Arm,
  Joint,
  IMU,
  ForceTorqueSensor,
  Task,
  Duration,
  defineBehavior,
  Sequence,
  Action,
  Condition,
} from '@hefaos/sdk';

// ─────────────────────────────────────────────────────────────────────────────
// Robot Definition
// ─────────────────────────────────────────────────────────────────────────────

export default defineRobot({
  name: 'SimpleArm',
  description: 'A 6-DOF robotic arm for pick and place operations',

  // Hardware components
  components: [
    // 6-DOF Arm
    <Arm name="arm" dof={6}>
      <Joint
        name="base"
        type="revolute"
        limits={[-180, 180]}
        maxVelocity={180}
        maxTorque={100}
      />
      <Joint
        name="shoulder"
        type="revolute"
        limits={[-90, 90]}
        maxVelocity={120}
        maxTorque={150}
      />
      <Joint
        name="elbow"
        type="revolute"
        limits={[-120, 120]}
        maxVelocity={180}
        maxTorque={80}
      />
      <Joint
        name="wrist1"
        type="revolute"
        limits={[-180, 180]}
        maxVelocity={360}
        maxTorque={20}
      />
      <Joint
        name="wrist2"
        type="revolute"
        limits={[-90, 90]}
        maxVelocity={360}
        maxTorque={20}
      />
      <Joint
        name="wrist3"
        type="revolute"
        limits={[-180, 180]}
        maxVelocity={360}
        maxTorque={20}
      />
    </Arm>,

    // IMU for base orientation
    <IMU
      name="base_imu"
      port="/dev/ttyUSB0"
      rate={1000}
      frame="base_link"
    />,

    // Force/torque sensor at end effector
    <ForceTorqueSensor
      name="ee_ft"
      port="/dev/ttyUSB1"
      rate={1000}
      frame="tool0"
    />,
  ],

  // Real-time tasks
  tasks: [
    // High-priority sensor reading (1kHz)
    <Task
      name="ReadSensors"
      priority="critical"
      period={Duration.ms(1)}
      deadline={Duration.us(500)}
      outputs={['imu_reading', 'joint_states', 'ft_reading']}
    />,

    // Control loop (1kHz, 800μs deadline)
    <Task
      name="ComputeControl"
      priority="critical"
      period={Duration.ms(1)}
      deadline={Duration.us(800)}
      inputs={['imu_reading', 'joint_states', 'ft_reading', 'target_pose']}
      outputs={['joint_commands']}
    />,

    // Actuator output (1kHz)
    <Task
      name="WriteActuators"
      priority="critical"
      period={Duration.ms(1)}
      inputs={['joint_commands']}
    />,

    // Motion planning (100Hz)
    <Task
      name="PlanMotion"
      priority="high"
      period={Duration.ms(10)}
      inputs={['goal_pose', 'joint_states']}
      outputs={['target_pose']}
    />,

    // Safety monitoring (1kHz)
    <Task
      name="SafetyMonitor"
      priority="critical"
      period={Duration.ms(1)}
      inputs={['joint_states', 'ft_reading']}
      outputs={['safety_status']}
    />,

    // Telemetry (10Hz)
    <Task
      name="PublishTelemetry"
      priority="low"
      period={Duration.ms(100)}
      inputs={['joint_states', 'safety_status']}
    />,
  ],

  // Default behavior
  defaultBehavior: 'idle',
});

// ─────────────────────────────────────────────────────────────────────────────
// Behaviors
// ─────────────────────────────────────────────────────────────────────────────

export const IdleBehavior = defineBehavior({
  name: 'idle',
  description: 'Hold current position',
  tree: (
    <Action name="HoldPosition" />
  ),
});

export const PickAndPlaceBehavior = defineBehavior({
  name: 'pick_and_place',
  description: 'Pick object from source and place at target',
  parameters: {
    pickPose: 'Pose',
    placePose: 'Pose',
  },
  tree: (
    <Sequence>
      {/* Approach pick position */}
      <Action name="MoveTo" args={{ pose: 'pickPose.approach' }} />

      {/* Move to pick position */}
      <Action name="MoveTo" args={{ pose: 'pickPose' }} />

      {/* Close gripper */}
      <Action name="CloseGripper" />

      {/* Wait for grasp confirmation */}
      <Condition name="GraspConfirmed" timeout={Duration.ms(500)} />

      {/* Retreat from pick */}
      <Action name="MoveTo" args={{ pose: 'pickPose.retreat' }} />

      {/* Move to place approach */}
      <Action name="MoveTo" args={{ pose: 'placePose.approach' }} />

      {/* Move to place position */}
      <Action name="MoveTo" args={{ pose: 'placePose' }} />

      {/* Open gripper */}
      <Action name="OpenGripper" />

      {/* Retreat from place */}
      <Action name="MoveTo" args={{ pose: 'placePose.retreat' }} />

      {/* Return to home */}
      <Action name="MoveToHome" />
    </Sequence>
  ),
});

export const HomingBehavior = defineBehavior({
  name: 'homing',
  description: 'Move all joints to home position',
  tree: (
    <Sequence>
      <Action name="EnableMotors" />
      <Action name="MoveToHome" speed={0.3} />
      <Action name="ZeroEncoders" />
    </Sequence>
  ),
});
