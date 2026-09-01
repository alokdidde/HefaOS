//! Versioned, transport-neutral contracts for the `HefaOS` SO-101 test bench.
//!
//! These types intentionally contain no executor, simulator, hardware, async,
//! or wall-clock dependency. Integer identity and time values serialize as
//! decimal strings so JSON consumers cannot silently lose `u64` precision.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const CONTRACT_SCHEMA_VERSION: u16 = 1;
pub const CONTRACT_ID: &str = "so101-bench/v0";
pub const PROFILE_ID: &str = "joint-position-loop/v0";
pub const ARM_JOINT_COUNT: usize = 5;
pub const SO101_TICK_PERIOD_NS: u64 = 5_000_000;
pub const PINNED_SO101_MODEL_DIGEST: &str =
    "sha256:5ad49f2b45c083baac9ffe5d4d3213a5da7eac8039095bb2df177a697aae8308";

macro_rules! decimal_u64 {
    ($name:ident) => {
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u64);

        impl $name {
            #[must_use]
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl From<u64> for $name {
            fn from(value: u64) -> Self {
                Self(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}", self.0)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.0.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let raw = String::deserialize(deserializer)?;
                raw.parse::<u64>()
                    .map(Self)
                    .map_err(serde::de::Error::custom)
            }
        }
    };
}

decimal_u64!(Tick);
decimal_u64!(VirtualTimeNs);
decimal_u64!(DurationNs);
decimal_u64!(ClockEpoch);
decimal_u64!(SourceEpoch);
decimal_u64!(PermitEpoch);
decimal_u64!(Sequence);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct So101StateV0 {
    pub arm_position_rad: [f64; ARM_JOINT_COUNT],
    pub arm_velocity_rad_s: [f64; ARM_JOINT_COUNT],
    pub gripper_position: f64,
    pub gripper_velocity_per_s: f64,
}

impl So101StateV0 {
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            arm_position_rad: [0.0; ARM_JOINT_COUNT],
            arm_velocity_rad_s: [0.0; ARM_JOINT_COUNT],
            gripper_position: 0.0,
            gripper_velocity_per_s: 0.0,
        }
    }

    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.arm_position_rad.iter().all(|value| value.is_finite())
            && self
                .arm_velocity_rad_s
                .iter()
                .all(|value| value.is_finite())
            && self.gripper_position.is_finite()
            && self.gripper_velocity_per_s.is_finite()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct So101CommandV0 {
    pub arm_position_rad: [f64; ARM_JOINT_COUNT],
    pub gripper_position: f64,
}

impl So101CommandV0 {
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            arm_position_rad: [0.0; ARM_JOINT_COUNT],
            gripper_position: 0.0,
        }
    }

    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.arm_position_rad.iter().all(|value| value.is_finite())
            && self.gripper_position.is_finite()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvalidReasonV0 {
    Missing,
    SensorFault,
    Uncalibrated,
    NonFinite,
    OutOfRange,
    WrongClockEpoch,
    WrongSourceEpoch,
    FutureTimestamp,
    Expired,
    Duplicate,
    OutOfOrder,
    TaskError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "reason", rename_all = "snake_case")]
pub enum ValidityV0 {
    Valid,
    Invalid(InvalidReasonV0),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvelopeV0<T> {
    pub clock_epoch: ClockEpoch,
    pub source_epoch: SourceEpoch,
    pub sequence: Sequence,
    pub captured_at_ns: VirtualTimeNs,
    pub valid_until_ns: VirtualTimeNs,
    pub validity: ValidityV0,
    pub payload: T,
}

pub type SensorFrameV0 = EnvelopeV0<So101StateV0>;
pub type SetpointFrameV0 = EnvelopeV0<So101CommandV0>;
pub type SafetyStatusFrameV0 = EnvelopeV0<SafetyStatusV0>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyTripReasonV0 {
    EmergencyStop,
    DriveFault,
    HeartbeatExpired,
    FeedbackInvalid,
    CommandInvalid,
    EpochMismatch,
    SubjectFault,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SafetyControllerStateV0 {
    Disarmed,
    Armed { permit_epoch: PermitEpoch },
    Tripped { reason: SafetyTripReasonV0 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SafetyStatusV0 {
    pub controller_state: SafetyControllerStateV0,
    pub interlocks_clear: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum SafetyActionV0 {
    ClearFaults,
    Reset,
    Arm,
    Disarm,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProposalFaultV0 {
    Expired,
    SourceSequenceMismatch,
    NonFinite,
    OutOfRange,
    TaskError,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubjectInputV0 {
    pub tick: Tick,
    pub time_ns: VirtualTimeNs,
    pub setpoint: Option<SetpointFrameV0>,
    pub sensor: Option<SensorFrameV0>,
    pub safety_status: Option<SafetyStatusFrameV0>,
    pub proposal_fault: Option<ProposalFaultV0>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlProposalV0 {
    pub clock_epoch: ClockEpoch,
    pub source_epoch: SourceEpoch,
    pub sequence: Sequence,
    pub source_sensor_epoch: SourceEpoch,
    pub source_sensor_sequence: Sequence,
    pub captured_at_ns: VirtualTimeNs,
    pub valid_until_ns: VirtualTimeNs,
    pub command: So101CommandV0,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActuatorIntentV0 {
    pub clock_epoch: ClockEpoch,
    pub source_epoch: SourceEpoch,
    pub sequence: Sequence,
    pub permit_epoch: PermitEpoch,
    pub captured_at_ns: VirtualTimeNs,
    pub valid_until_ns: VirtualTimeNs,
    pub command: So101CommandV0,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateDeniedReasonV0 {
    MissingSensor,
    InvalidSensor,
    StaleSensor,
    FutureSensor,
    DuplicateSensor,
    OutOfOrderSensor,
    WrongClockEpoch,
    WrongSourceEpoch,
    MissingSetpoint,
    InvalidSetpoint,
    StaleSetpoint,
    MissingSafetyStatus,
    InvalidSafetyStatus,
    StaleSafetyStatus,
    SafetyNotArmed,
    SafetyTripped,
    PermitEpochMismatch,
    ProposalExpired,
    ProposalSourceMismatch,
    NonFinite,
    OutOfRange,
    TaskError,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum GateDecisionV0 {
    NoCommand,
    Denied {
        reason: GateDeniedReasonV0,
    },
    Admitted {
        intent: ActuatorIntentV0,
        limited: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SubjectLifecycleV0 {
    Running,
    Faulted { reason: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubjectOutputV0 {
    pub lifecycle: SubjectLifecycleV0,
    pub estimate: Option<So101StateV0>,
    pub control: Option<ControlProposalV0>,
    pub gate: GateDecisionV0,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppliedActuationV0 {
    pub tick: Tick,
    pub time_ns: VirtualTimeNs,
    pub permit_epoch: PermitEpoch,
    pub intent_sequence: Sequence,
    pub command: So101CommandV0,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuppressionReasonV0 {
    Disarmed,
    Tripped,
    ArmTransition,
    FeedbackInvalid,
    MissingIntent,
    ExpiredIntent,
    WrongClockEpoch,
    WrongSourceEpoch,
    WrongPermitEpoch,
    DuplicateIntent,
    OutOfOrderIntent,
    SlewLimitExceeded,
    NonFinite,
    OutOfRange,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "disposition", rename_all = "snake_case")]
pub enum SafetyDispositionV0 {
    Authorized { actuation: AppliedActuationV0 },
    Suppressed { reason: SuppressionReasonV0 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JointLimitsV0 {
    pub arm_position_rad: [[f64; 2]; ARM_JOINT_COUNT],
    pub gripper_position: [f64; 2],
}

impl JointLimitsV0 {
    #[must_use]
    pub fn contains_state(&self, state: &So101StateV0) -> bool {
        state.is_finite()
            && state
                .arm_position_rad
                .iter()
                .zip(&self.arm_position_rad)
                .all(|(value, [minimum, maximum])| (*minimum..=*maximum).contains(value))
            && (self.gripper_position[0]..=self.gripper_position[1])
                .contains(&state.gripper_position)
    }

    #[must_use]
    pub fn contains_command(&self, command: &So101CommandV0) -> bool {
        command.is_finite()
            && command
                .arm_position_rad
                .iter()
                .zip(&self.arm_position_rad)
                .all(|(value, [minimum, maximum])| (*minimum..=*maximum).contains(value))
            && (self.gripper_position[0]..=self.gripper_position[1])
                .contains(&command.gripper_position)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GatePolicyV0 {
    pub sensor_max_age_ns: DurationNs,
    pub safety_max_age_ns: DurationNs,
    pub setpoint_max_age_ns: DurationNs,
    pub proposal_ttl_ns: DurationNs,
    pub max_arm_step_rad: [f64; ARM_JOINT_COUNT],
    pub max_gripper_step: f64,
    pub limits: JointLimitsV0,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SafetyPolicyV0 {
    pub intent_max_age_ns: DurationNs,
    pub heartbeat_timeout_ns: DurationNs,
    pub max_arm_step_rad: [f64; ARM_JOINT_COUNT],
    pub max_gripper_step: f64,
    pub limits: JointLimitsV0,
}

/// The runtime configuration visible to a subject under test.
///
/// This deliberately excludes expected outcomes, future faults and safety
/// actions, plant policy, equality thresholds, and other oracle-owned data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubjectConfigV0 {
    pub schema_version: u16,
    pub contract: String,
    pub profile: String,
    pub clock_epoch: ClockEpoch,
    pub sensor_source_epoch: SourceEpoch,
    pub setpoint_source_epoch: SourceEpoch,
    pub subject_source_epoch: SourceEpoch,
    pub safety_source_epoch: SourceEpoch,
    pub tick_period_ns: DurationNs,
    pub initial_state: So101StateV0,
    pub gate_policy: GatePolicyV0,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MockPlantPolicyV0 {
    pub max_arm_velocity_rad_s: [f64; ARM_JOINT_COUNT],
    pub max_gripper_velocity_per_s: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetpointKeyframeV0 {
    pub at_tick: Tick,
    pub command: So101CommandV0,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduledSafetyActionV0 {
    pub at_tick: Tick,
    pub action: SafetyActionV0,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FaultKindV0 {
    DropSensor,
    InvalidateSensor { reason: InvalidReasonV0 },
    OutOfRangeSensor,
    FutureSensor { offset_ns: DurationNs },
    OverAgeSensor { age_ns: DurationNs },
    StaleSensor { age_ns: DurationNs },
    DuplicateSensor,
    ReorderSensor,
    WrongSensorClockEpoch,
    WrongSensorSourceEpoch,
    DropSetpoint,
    InvalidateSetpoint { reason: InvalidReasonV0 },
    OutOfRangeSetpoint,
    FutureSetpoint { offset_ns: DurationNs },
    OverAgeSetpoint { age_ns: DurationNs },
    StaleSetpoint { age_ns: DurationNs },
    DuplicateSetpoint,
    ReorderSetpoint,
    WrongSetpointClockEpoch,
    WrongSetpointSourceEpoch,
    DropSafetyStatus,
    InvalidateSafetyStatus { reason: InvalidReasonV0 },
    OpenSafetyInterlock,
    FutureSafetyStatus { offset_ns: DurationNs },
    OverAgeSafetyStatus { age_ns: DurationNs },
    StaleSafetyStatus { age_ns: DurationNs },
    DuplicateSafetyStatus,
    ReorderSafetyStatus,
    WrongSafetyClockEpoch,
    WrongSafetySourceEpoch,
    WrongSafetyPermitEpoch,
    RevokePermit,
    EmergencyStop,
    DriveFault,
    Proposal { fault: ProposalFaultV0 },
    DropIntent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduledFaultV0 {
    pub start_tick: Tick,
    pub duration_ticks: Tick,
    pub fault: FaultKindV0,
}

impl ScheduledFaultV0 {
    #[must_use]
    pub fn active_at(&self, tick: Tick) -> bool {
        let end = self.start_tick.0.saturating_add(self.duration_ticks.0);
        tick.0 >= self.start_tick.0 && tick.0 < end
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedSafetyStateV0 {
    Disarmed,
    Armed,
    Tripped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpectedStateV0 {
    pub state: So101StateV0,
    pub absolute_tolerance: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpectedRunV0 {
    pub admitted_ticks: Option<u64>,
    pub denied_by_reason: BTreeMap<GateDeniedReasonV0, u64>,
    pub authorized_ticks: Option<u64>,
    pub suppressed_ticks: Vec<Tick>,
    pub terminal_safety_state: Option<ExpectedSafetyStateV0>,
    pub final_state: Option<ExpectedStateV0>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EqualityProfileV0 {
    pub control_absolute_tolerance: f64,
    pub physics_absolute_tolerance: f64,
    pub require_exact_discrete_trace: bool,
    pub require_bit_exact_mock_trace: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScenarioV0 {
    pub schema_version: u16,
    pub contract: String,
    pub profile: String,
    pub name: String,
    pub model_digest: String,
    pub clock_epoch: ClockEpoch,
    pub sensor_source_epoch: SourceEpoch,
    pub setpoint_source_epoch: SourceEpoch,
    pub subject_source_epoch: SourceEpoch,
    pub safety_source_epoch: SourceEpoch,
    pub tick_period_ns: DurationNs,
    pub ticks: Tick,
    pub seed: String,
    pub initial_state: So101StateV0,
    pub setpoints: Vec<SetpointKeyframeV0>,
    pub safety_actions: Vec<ScheduledSafetyActionV0>,
    pub faults: Vec<ScheduledFaultV0>,
    pub gate_policy: GatePolicyV0,
    pub safety_policy: SafetyPolicyV0,
    pub mock_plant_policy: MockPlantPolicyV0,
    pub equality: EqualityProfileV0,
    pub expected: ExpectedRunV0,
}

impl ScenarioV0 {
    /// Returns only the configuration that a black-box subject may observe.
    #[must_use]
    pub fn subject_config(&self) -> SubjectConfigV0 {
        SubjectConfigV0 {
            schema_version: self.schema_version,
            contract: self.contract.clone(),
            profile: self.profile.clone(),
            clock_epoch: self.clock_epoch,
            sensor_source_epoch: self.sensor_source_epoch,
            setpoint_source_epoch: self.setpoint_source_epoch,
            subject_source_epoch: self.subject_source_epoch,
            safety_source_epoch: self.safety_source_epoch,
            tick_period_ns: self.tick_period_ns,
            initial_state: self.initial_state.clone(),
            gate_policy: self.gate_policy.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TraceRecordV0 {
    pub tick: Tick,
    pub time_ns: VirtualTimeNs,
    pub safety_actions: Vec<SafetyActionV0>,
    pub active_faults: Vec<FaultKindV0>,
    /// Authoritative pre-actuation plant state observed by the independent
    /// safety oracle. This is deliberately separate from the fault-injected
    /// sensor frame delivered to the subject.
    pub safety_observation: So101StateV0,
    pub subject_input: SubjectInputV0,
    pub subject_output: SubjectOutputV0,
    pub safety_disposition: SafetyDispositionV0,
    pub safety_controller_after: SafetyStatusV0,
    pub plant_state_after: So101StateV0,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunSummaryV0 {
    pub admitted_ticks: u64,
    pub denied_by_reason: BTreeMap<GateDeniedReasonV0, u64>,
    pub authorized_ticks: u64,
    pub suppressed_ticks: Vec<Tick>,
    pub terminal_safety_state: ExpectedSafetyStateV0,
    pub replayable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticTraceV0 {
    pub schema_version: u16,
    pub contract: String,
    pub profile: String,
    pub scenario_name: String,
    pub scenario_sha256: String,
    pub model_digest: String,
    pub seed: String,
    pub tick_period_ns: DurationNs,
    pub expected_ticks: Tick,
    pub subject_config: SubjectConfigV0,
    pub safety_policy: SafetyPolicyV0,
    pub subject_id: String,
    pub plant_id: String,
    pub equality: EqualityProfileV0,
    pub records: Vec<TraceRecordV0>,
    pub summary: RunSummaryV0,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LatencySummaryV0 {
    pub samples: u64,
    pub p50_ns: u64,
    pub p95_ns: u64,
    pub p99_ns: u64,
    pub p999_ns: u64,
    pub maximum_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BenchmarkReportV0 {
    pub schema_version: u16,
    pub scenario_name: String,
    pub scenario_sha256: String,
    pub subject_id: String,
    pub plant_id: String,
    pub iterations: u64,
    pub semantic_failures: u64,
    pub control_turn_latency: LatencySummaryV0,
    pub wall_time_is_portable_gate: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_envelopes_serialize_losslessly_as_strings() {
        let value = VirtualTimeNs(u64::MAX);
        let json = serde_json::to_string(&value).expect("serialize virtual time");
        assert_eq!(json, format!("\"{}\"", u64::MAX));
        assert_eq!(
            serde_json::from_str::<VirtualTimeNs>(&json).expect("deserialize virtual time"),
            value
        );
    }

    #[test]
    fn validity_interval_is_representable_above_javascript_integer_range() {
        let frame = EnvelopeV0 {
            clock_epoch: ClockEpoch(9_007_199_254_740_993),
            source_epoch: SourceEpoch(2),
            sequence: Sequence(3),
            captured_at_ns: VirtualTimeNs(9_007_199_254_740_994),
            valid_until_ns: VirtualTimeNs(9_007_199_254_740_995),
            validity: ValidityV0::Valid,
            payload: So101CommandV0::zero(),
        };
        let encoded = serde_json::to_string(&frame).expect("serialize frame");
        let decoded: SetpointFrameV0 = serde_json::from_str(&encoded).expect("deserialize frame");
        assert_eq!(decoded, frame);
    }
}
