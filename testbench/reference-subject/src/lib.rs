//! Small deterministic subject used as the testbench's semantic oracle.
//!
//! The reference subject deliberately has no executor, physics, hardware,
//! async, or wall-clock dependency. It estimates by copying a validated sensor
//! frame, proposes the validated position setpoint, and admits at most one
//! bounded actuator intent through a stateful software motion gate.

use hefaos_testbench_contracts::{
    ARM_JOINT_COUNT, ActuatorIntentV0, CONTRACT_ID, CONTRACT_SCHEMA_VERSION, ClockEpoch,
    ControlProposalV0, DurationNs, EnvelopeV0, GateDecisionV0, GateDeniedReasonV0, GatePolicyV0,
    InvalidReasonV0, PROFILE_ID, PermitEpoch, ProposalFaultV0, SafetyControllerStateV0,
    SafetyStatusFrameV0, SensorFrameV0, Sequence, SetpointFrameV0, So101CommandV0, So101StateV0,
    SourceEpoch, SubjectConfigV0, SubjectInputV0, SubjectLifecycleV0, SubjectOutputV0, ValidityV0,
    VirtualTimeNs,
};
use hefaos_testbench_harness::{Subject, SubjectError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable identifier recorded in semantic traces.
pub const REFERENCE_SUBJECT_ID: &str = "reference-subject/v0";

/// Configuration or lifecycle errors at the reference-subject boundary.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReferenceSubjectError {
    /// `step` was called before a successful reset.
    #[error("reference subject must be reset with a valid scenario before stepping")]
    NotReset,
    /// The scenario cannot configure a safe deterministic motion gate.
    #[error("invalid reference-subject scenario: {0}")]
    InvalidScenario(String),
    /// A serialized subject snapshot cannot safely resume this subject.
    #[error("invalid reference-subject snapshot: {0}")]
    InvalidSnapshot(String),
}

#[derive(Debug, Clone)]
struct RuntimeConfig {
    clock_epoch: ClockEpoch,
    sensor_source_epoch: SourceEpoch,
    setpoint_source_epoch: SourceEpoch,
    subject_source_epoch: SourceEpoch,
    safety_source_epoch: SourceEpoch,
    policy: GatePolicyV0,
}

/// Fixed, executor-neutral runtime configuration needed to resume a subject.
///
/// This deliberately records only fields that influence `ReferenceSubject`
/// after reset. Scenario identity and initial plant state have already been
/// validated at reset and do not affect subsequent subject turns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReferenceSubjectRuntimeConfigV0 {
    pub clock_epoch: ClockEpoch,
    pub sensor_source_epoch: SourceEpoch,
    pub setpoint_source_epoch: SourceEpoch,
    pub subject_source_epoch: SourceEpoch,
    pub safety_source_epoch: SourceEpoch,
    pub policy: GatePolicyV0,
}

impl From<&RuntimeConfig> for ReferenceSubjectRuntimeConfigV0 {
    fn from(config: &RuntimeConfig) -> Self {
        Self {
            clock_epoch: config.clock_epoch,
            sensor_source_epoch: config.sensor_source_epoch,
            setpoint_source_epoch: config.setpoint_source_epoch,
            subject_source_epoch: config.subject_source_epoch,
            safety_source_epoch: config.safety_source_epoch,
            policy: config.policy.clone(),
        }
    }
}

impl From<ReferenceSubjectRuntimeConfigV0> for RuntimeConfig {
    fn from(config: ReferenceSubjectRuntimeConfigV0) -> Self {
        Self {
            clock_epoch: config.clock_epoch,
            sensor_source_epoch: config.sensor_source_epoch,
            setpoint_source_epoch: config.setpoint_source_epoch,
            subject_source_epoch: config.subject_source_epoch,
            safety_source_epoch: config.safety_source_epoch,
            policy: config.policy,
        }
    }
}

/// Bounded terminal faults that may be latched by the reference subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceSubjectFaultV0 {
    ControllerTaskError,
}

impl ReferenceSubjectFaultV0 {
    const fn reason(self) -> &'static str {
        match self {
            Self::ControllerTaskError => "injected deterministic controller task error",
        }
    }
}

/// Complete fixed-size state needed to resume deterministic subject execution.
///
/// The snapshot retains no input history. It is intentionally executor-neutral:
/// adapters may encode it with their own checkpoint mechanism without pulling
/// executor types into the testbench contract or reference subject.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReferenceSubjectSnapshotV0 {
    pub config: Option<ReferenceSubjectRuntimeConfigV0>,
    pub sensor_cursor: Option<Sequence>,
    pub setpoint_cursor: Option<Sequence>,
    pub safety_cursor: Option<Sequence>,
    pub next_proposal_sequence: u64,
    pub active_permit_epoch: Option<PermitEpoch>,
    pub fault_latch: Option<ReferenceSubjectFaultV0>,
}

#[derive(Debug, Default, Clone, Copy)]
struct StreamCursor {
    last: Option<Sequence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CursorError {
    Duplicate,
    OutOfOrder,
}

impl StreamCursor {
    fn observe(&mut self, sequence: Sequence) -> Result<(), CursorError> {
        match self.last {
            None => {
                self.last = Some(sequence);
                Ok(())
            }
            Some(last) if sequence > last => {
                // Loss is diagnosed by the harness trace, but the newest
                // monotonic sample remains admissible. This lets a stream
                // recover after a deliberately dropped or corrupted frame.
                self.last = Some(sequence);
                Ok(())
            }
            Some(last) if sequence == last => Err(CursorError::Duplicate),
            Some(_) => Err(CursorError::OutOfOrder),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum EnvelopeError {
    WrongClockEpoch,
    WrongSourceEpoch,
    Duplicate,
    OutOfOrder,
    Invalid(InvalidReasonV0),
    Future,
    Stale,
}

/// Deterministic pass-through estimator, position controller, and motion gate.
#[derive(Debug, Default)]
pub struct ReferenceSubject {
    config: Option<RuntimeConfig>,
    sensor_cursor: StreamCursor,
    setpoint_cursor: StreamCursor,
    safety_cursor: StreamCursor,
    next_proposal_sequence: u64,
    active_permit_epoch: Option<PermitEpoch>,
    faulted: Option<ReferenceSubjectFaultV0>,
}

impl ReferenceSubject {
    /// Creates an unconfigured subject. Call [`Self::reset`] before stepping it.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            config: None,
            sensor_cursor: StreamCursor { last: None },
            setpoint_cursor: StreamCursor { last: None },
            safety_cursor: StreamCursor { last: None },
            next_proposal_sequence: 0,
            active_permit_epoch: None,
            faulted: None,
        }
    }

    /// Returns the stable subject identifier used by traces and comparisons.
    #[must_use]
    pub const fn id(&self) -> &'static str {
        REFERENCE_SUBJECT_ID
    }

    /// Returns a complete, bounded checkpoint of deterministic subject state.
    #[must_use]
    pub fn snapshot(&self) -> ReferenceSubjectSnapshotV0 {
        ReferenceSubjectSnapshotV0 {
            config: self
                .config
                .as_ref()
                .map(ReferenceSubjectRuntimeConfigV0::from),
            sensor_cursor: self.sensor_cursor.last,
            setpoint_cursor: self.setpoint_cursor.last,
            safety_cursor: self.safety_cursor.last,
            next_proposal_sequence: self.next_proposal_sequence,
            active_permit_epoch: self.active_permit_epoch,
            fault_latch: self.faulted,
        }
    }

    /// Atomically restores a checkpoint created by [`Self::snapshot`].
    ///
    /// # Errors
    ///
    /// Returns [`ReferenceSubjectError::InvalidSnapshot`] when the checkpoint
    /// cannot describe a valid configured or unconfigured subject. On error,
    /// this subject is left unchanged.
    pub fn restore(
        &mut self,
        snapshot: &ReferenceSubjectSnapshotV0,
    ) -> Result<(), ReferenceSubjectError> {
        validate_snapshot(snapshot)?;

        self.config = snapshot.config.clone().map(RuntimeConfig::from);
        self.sensor_cursor.last = snapshot.sensor_cursor;
        self.setpoint_cursor.last = snapshot.setpoint_cursor;
        self.safety_cursor.last = snapshot.safety_cursor;
        self.next_proposal_sequence = snapshot.next_proposal_sequence;
        self.active_permit_epoch = snapshot.active_permit_epoch;
        self.faulted = snapshot.fault_latch;
        Ok(())
    }

    /// Resets every stateful cursor, permit, rate limiter, and fault latch.
    ///
    /// # Errors
    ///
    /// Returns [`ReferenceSubjectError::InvalidScenario`] if the scenario's
    /// identity, initial state, limits, TTLs, or rate limits are unsafe.
    pub fn reset(&mut self, config: &SubjectConfigV0) -> Result<(), ReferenceSubjectError> {
        validate_config(config)?;

        self.config = Some(RuntimeConfig {
            clock_epoch: config.clock_epoch,
            sensor_source_epoch: config.sensor_source_epoch,
            setpoint_source_epoch: config.setpoint_source_epoch,
            subject_source_epoch: config.subject_source_epoch,
            safety_source_epoch: config.safety_source_epoch,
            policy: config.gate_policy.clone(),
        });
        self.sensor_cursor = StreamCursor::default();
        self.setpoint_cursor = StreamCursor::default();
        self.safety_cursor = StreamCursor::default();
        self.next_proposal_sequence = 0;
        self.active_permit_epoch = None;
        self.faulted = None;
        Ok(())
    }

    /// Processes one harness-scheduled virtual turn without reading wall time.
    ///
    /// # Errors
    ///
    /// Returns [`ReferenceSubjectError::NotReset`] before a successful reset,
    /// or an invalid-scenario error if the proposal sequence is exhausted.
    #[allow(clippy::too_many_lines)]
    pub fn step(
        &mut self,
        input: &SubjectInputV0,
    ) -> Result<SubjectOutputV0, ReferenceSubjectError> {
        let config = self.config.clone().ok_or(ReferenceSubjectError::NotReset)?;

        if let Some(fault) = self.faulted {
            return Ok(faulted_output(fault));
        }

        // Observe all three independent streams on every turn. A denial in one
        // stream must not manufacture sequence gaps in the other two streams.
        let sensor = validate_sensor(
            input.sensor.as_ref(),
            input.time_ns,
            &config,
            &mut self.sensor_cursor,
        );
        let setpoint = validate_setpoint(
            input.setpoint.as_ref(),
            input.time_ns,
            &config,
            &mut self.setpoint_cursor,
        );
        let safety = validate_safety(
            input.safety_status.as_ref(),
            input.time_ns,
            &config,
            &mut self.safety_cursor,
        );

        let estimate = sensor.as_ref().ok().map(|frame| frame.payload.clone());

        let sensor = match sensor {
            Ok(frame) => frame,
            Err(reason) => return Ok(denied_output(estimate, None, reason)),
        };
        let setpoint = match setpoint {
            Ok(frame) => frame,
            Err(reason) => return Ok(denied_output(estimate, None, reason)),
        };
        let safety = match safety {
            Ok(frame) => frame,
            Err(reason) => return Ok(denied_output(estimate, None, reason)),
        };

        if matches!(input.proposal_fault, Some(ProposalFaultV0::TaskError)) {
            let fault = ReferenceSubjectFaultV0::ControllerTaskError;
            self.faulted = Some(fault);
            return Ok(SubjectOutputV0 {
                lifecycle: SubjectLifecycleV0::Faulted {
                    reason: fault.reason().to_owned(),
                },
                estimate,
                control: None,
                gate: GateDecisionV0::Denied {
                    reason: GateDeniedReasonV0::TaskError,
                },
            });
        }

        let proposal_sequence = Sequence(self.next_proposal_sequence);
        self.next_proposal_sequence =
            self.next_proposal_sequence.checked_add(1).ok_or_else(|| {
                ReferenceSubjectError::InvalidScenario("proposal sequence exhausted".to_owned())
            })?;

        let mut proposal = ControlProposalV0 {
            clock_epoch: config.clock_epoch,
            source_epoch: config.subject_source_epoch,
            sequence: proposal_sequence,
            source_sensor_epoch: sensor.source_epoch,
            source_sensor_sequence: sensor.sequence,
            captured_at_ns: input.time_ns,
            valid_until_ns: VirtualTimeNs(
                effective_deadline(sensor, config.policy.sensor_max_age_ns)
                    .min(effective_deadline(
                        setpoint,
                        config.policy.setpoint_max_age_ns,
                    ))
                    .min(
                        input
                            .time_ns
                            .0
                            .saturating_add(config.policy.proposal_ttl_ns.0),
                    ),
            ),
            command: setpoint.payload.clone(),
        };

        apply_proposal_fault(&mut proposal, input.proposal_fault.as_ref(), &config.policy);
        // Contract payloads are finite-only so evidence remains valid JSON. The
        // injected directive in SubjectInputV0 is the evidence for a synthetic
        // non-finite proposal; the rejected payload itself is not serialized.
        let control = if matches!(input.proposal_fault, Some(ProposalFaultV0::NonFinite)) {
            None
        } else {
            Some(proposal.clone())
        };

        if input.time_ns.0 >= proposal.valid_until_ns.0 {
            return Ok(denied_output(
                estimate,
                control,
                GateDeniedReasonV0::ProposalExpired,
            ));
        }
        if proposal.source_sensor_epoch != sensor.source_epoch
            || proposal.source_sensor_sequence != sensor.sequence
        {
            return Ok(denied_output(
                estimate,
                control,
                GateDeniedReasonV0::ProposalSourceMismatch,
            ));
        }
        if !proposal.command.is_finite() {
            return Ok(denied_output(
                estimate,
                control,
                GateDeniedReasonV0::NonFinite,
            ));
        }
        if !config.policy.limits.contains_command(&proposal.command) {
            return Ok(denied_output(
                estimate,
                control,
                GateDeniedReasonV0::OutOfRange,
            ));
        }

        let permit_epoch = match &safety.payload.controller_state {
            SafetyControllerStateV0::Disarmed => {
                self.active_permit_epoch = None;
                return Ok(denied_output(
                    estimate,
                    control,
                    GateDeniedReasonV0::SafetyNotArmed,
                ));
            }
            SafetyControllerStateV0::Tripped { .. } => {
                self.active_permit_epoch = None;
                return Ok(denied_output(
                    estimate,
                    control,
                    GateDeniedReasonV0::SafetyTripped,
                ));
            }
            SafetyControllerStateV0::Armed { permit_epoch } => *permit_epoch,
        };

        if !safety.payload.interlocks_clear {
            self.active_permit_epoch = None;
            return Ok(denied_output(
                estimate,
                control,
                GateDeniedReasonV0::InvalidSafetyStatus,
            ));
        }

        if let Some(active) = self.active_permit_epoch {
            if active != permit_epoch {
                return Ok(denied_output(
                    estimate,
                    control,
                    GateDeniedReasonV0::PermitEpochMismatch,
                ));
            }
        } else {
            self.active_permit_epoch = Some(permit_epoch);
        }

        let observed_command = So101CommandV0 {
            arm_position_rad: sensor.payload.arm_position_rad,
            gripper_position: sensor.payload.gripper_position,
        };
        let (bounded_command, limited) =
            rate_limit(&proposal.command, &observed_command, &config.policy);
        let intent = ActuatorIntentV0 {
            clock_epoch: proposal.clock_epoch,
            source_epoch: proposal.source_epoch,
            sequence: proposal.sequence,
            permit_epoch,
            captured_at_ns: proposal.captured_at_ns,
            valid_until_ns: VirtualTimeNs(
                proposal
                    .valid_until_ns
                    .0
                    .min(effective_deadline(safety, config.policy.safety_max_age_ns)),
            ),
            command: bounded_command.clone(),
        };

        if input.time_ns.0 >= intent.valid_until_ns.0 {
            return Ok(denied_output(
                estimate,
                control,
                GateDeniedReasonV0::StaleSafetyStatus,
            ));
        }

        Ok(SubjectOutputV0 {
            lifecycle: SubjectLifecycleV0::Running,
            estimate,
            control,
            gate: GateDecisionV0::Admitted { intent, limited },
        })
    }
}

impl Subject for ReferenceSubject {
    fn id(&self) -> &'static str {
        ReferenceSubject::id(self)
    }

    fn reset(&mut self, config: &SubjectConfigV0) -> Result<(), SubjectError> {
        ReferenceSubject::reset(self, config)
            .map_err(|error| SubjectError::Reset(error.to_string()))
    }

    fn step(&mut self, input: &SubjectInputV0) -> Result<SubjectOutputV0, SubjectError> {
        ReferenceSubject::step(self, input).map_err(|error| SubjectError::Step(error.to_string()))
    }
}

fn validate_config(config: &SubjectConfigV0) -> Result<(), ReferenceSubjectError> {
    if config.schema_version != CONTRACT_SCHEMA_VERSION {
        return Err(ReferenceSubjectError::InvalidScenario(format!(
            "schema version {} does not match {CONTRACT_SCHEMA_VERSION}",
            config.schema_version
        )));
    }
    if config.contract != CONTRACT_ID {
        return Err(ReferenceSubjectError::InvalidScenario(format!(
            "contract {:?} does not match {CONTRACT_ID:?}",
            config.contract
        )));
    }
    if config.profile != PROFILE_ID {
        return Err(ReferenceSubjectError::InvalidScenario(format!(
            "profile {:?} does not match {PROFILE_ID:?}",
            config.profile
        )));
    }

    validate_runtime_policy(&config.gate_policy).map_err(ReferenceSubjectError::InvalidScenario)?;
    let policy = &config.gate_policy;
    if !config.initial_state.is_finite() || !policy.limits.contains_state(&config.initial_state) {
        return Err(ReferenceSubjectError::InvalidScenario(
            "initial state must be finite and inside gate limits".to_owned(),
        ));
    }
    Ok(())
}

fn validate_runtime_policy(policy: &GatePolicyV0) -> Result<(), String> {
    if policy.sensor_max_age_ns.0 == 0
        || policy.safety_max_age_ns.0 == 0
        || policy.setpoint_max_age_ns.0 == 0
        || policy.proposal_ttl_ns.0 == 0
    {
        return Err("gate TTLs and maximum ages must be non-zero".to_owned());
    }
    if !valid_limits(
        &policy.limits.arm_position_rad,
        policy.limits.gripper_position,
    ) {
        return Err("joint limits must be finite ordered inclusive ranges".to_owned());
    }
    if policy
        .max_arm_step_rad
        .iter()
        .any(|step| !step.is_finite() || *step < 0.0)
        || !policy.max_gripper_step.is_finite()
        || policy.max_gripper_step < 0.0
    {
        return Err("rate-limit steps must be finite and non-negative".to_owned());
    }
    Ok(())
}

fn validate_snapshot(snapshot: &ReferenceSubjectSnapshotV0) -> Result<(), ReferenceSubjectError> {
    match &snapshot.config {
        Some(config) => {
            validate_runtime_policy(&config.policy).map_err(ReferenceSubjectError::InvalidSnapshot)
        }
        None if snapshot.sensor_cursor.is_none()
            && snapshot.setpoint_cursor.is_none()
            && snapshot.safety_cursor.is_none()
            && snapshot.next_proposal_sequence == 0
            && snapshot.active_permit_epoch.is_none()
            && snapshot.fault_latch.is_none() =>
        {
            Ok(())
        }
        None => Err(ReferenceSubjectError::InvalidSnapshot(
            "unconfigured subject cannot retain deterministic runtime state".to_owned(),
        )),
    }
}

fn valid_limits(arm: &[[f64; 2]; ARM_JOINT_COUNT], gripper: [f64; 2]) -> bool {
    arm.iter()
        .chain(std::iter::once(&gripper))
        .all(|[minimum, maximum]| minimum.is_finite() && maximum.is_finite() && minimum <= maximum)
}

fn validate_sensor<'a>(
    frame: Option<&'a SensorFrameV0>,
    now: VirtualTimeNs,
    config: &RuntimeConfig,
    cursor: &mut StreamCursor,
) -> Result<&'a SensorFrameV0, GateDeniedReasonV0> {
    let frame = frame.ok_or(GateDeniedReasonV0::MissingSensor)?;
    validate_envelope(
        frame,
        now,
        config.clock_epoch,
        config.sensor_source_epoch,
        config.policy.sensor_max_age_ns,
        cursor,
    )
    .map_err(|error| sensor_envelope_reason(&error))?;

    if !frame.payload.is_finite() {
        return Err(GateDeniedReasonV0::NonFinite);
    }
    if !config.policy.limits.contains_state(&frame.payload) {
        return Err(GateDeniedReasonV0::OutOfRange);
    }
    Ok(frame)
}

fn validate_setpoint<'a>(
    frame: Option<&'a SetpointFrameV0>,
    now: VirtualTimeNs,
    config: &RuntimeConfig,
    cursor: &mut StreamCursor,
) -> Result<&'a SetpointFrameV0, GateDeniedReasonV0> {
    let frame = frame.ok_or(GateDeniedReasonV0::MissingSetpoint)?;
    validate_envelope(
        frame,
        now,
        config.clock_epoch,
        config.setpoint_source_epoch,
        config.policy.setpoint_max_age_ns,
        cursor,
    )
    .map_err(|error| setpoint_envelope_reason(&error))?;

    if !frame.payload.is_finite() {
        return Err(GateDeniedReasonV0::NonFinite);
    }
    if !config.policy.limits.contains_command(&frame.payload) {
        return Err(GateDeniedReasonV0::OutOfRange);
    }
    Ok(frame)
}

fn validate_safety<'a>(
    frame: Option<&'a SafetyStatusFrameV0>,
    now: VirtualTimeNs,
    config: &RuntimeConfig,
    cursor: &mut StreamCursor,
) -> Result<&'a SafetyStatusFrameV0, GateDeniedReasonV0> {
    let frame = frame.ok_or(GateDeniedReasonV0::MissingSafetyStatus)?;
    validate_envelope(
        frame,
        now,
        config.clock_epoch,
        config.safety_source_epoch,
        config.policy.safety_max_age_ns,
        cursor,
    )
    .map_err(|error| safety_envelope_reason(&error))?;
    Ok(frame)
}

fn validate_envelope<T>(
    frame: &EnvelopeV0<T>,
    now: VirtualTimeNs,
    expected_clock_epoch: ClockEpoch,
    expected_source_epoch: SourceEpoch,
    maximum_age: DurationNs,
    cursor: &mut StreamCursor,
) -> Result<(), EnvelopeError> {
    if frame.clock_epoch != expected_clock_epoch {
        return Err(EnvelopeError::WrongClockEpoch);
    }
    if frame.source_epoch != expected_source_epoch {
        return Err(EnvelopeError::WrongSourceEpoch);
    }
    if frame.captured_at_ns.0 > now.0 {
        return Err(EnvelopeError::Future);
    }
    cursor
        .observe(frame.sequence)
        .map_err(|error| match error {
            CursorError::Duplicate => EnvelopeError::Duplicate,
            CursorError::OutOfOrder => EnvelopeError::OutOfOrder,
        })?;
    if let ValidityV0::Invalid(reason) = &frame.validity {
        return Err(EnvelopeError::Invalid(reason.clone()));
    }
    if now.0 >= effective_deadline(frame, maximum_age) {
        return Err(EnvelopeError::Stale);
    }
    Ok(())
}

fn effective_deadline<T>(frame: &EnvelopeV0<T>, maximum_age: DurationNs) -> u64 {
    frame
        .valid_until_ns
        .0
        .min(frame.captured_at_ns.0.saturating_add(maximum_age.0))
}

fn sensor_envelope_reason(error: &EnvelopeError) -> GateDeniedReasonV0 {
    match error {
        EnvelopeError::WrongClockEpoch
        | EnvelopeError::Invalid(InvalidReasonV0::WrongClockEpoch) => {
            GateDeniedReasonV0::WrongClockEpoch
        }
        EnvelopeError::WrongSourceEpoch
        | EnvelopeError::Invalid(InvalidReasonV0::WrongSourceEpoch) => {
            GateDeniedReasonV0::WrongSourceEpoch
        }
        EnvelopeError::Future | EnvelopeError::Invalid(InvalidReasonV0::FutureTimestamp) => {
            GateDeniedReasonV0::FutureSensor
        }
        EnvelopeError::Stale | EnvelopeError::Invalid(InvalidReasonV0::Expired) => {
            GateDeniedReasonV0::StaleSensor
        }
        EnvelopeError::Duplicate | EnvelopeError::Invalid(InvalidReasonV0::Duplicate) => {
            GateDeniedReasonV0::DuplicateSensor
        }
        EnvelopeError::OutOfOrder | EnvelopeError::Invalid(InvalidReasonV0::OutOfOrder) => {
            GateDeniedReasonV0::OutOfOrderSensor
        }
        EnvelopeError::Invalid(InvalidReasonV0::NonFinite) => GateDeniedReasonV0::NonFinite,
        EnvelopeError::Invalid(InvalidReasonV0::OutOfRange) => GateDeniedReasonV0::OutOfRange,
        EnvelopeError::Invalid(_) => GateDeniedReasonV0::InvalidSensor,
    }
}

fn setpoint_envelope_reason(error: &EnvelopeError) -> GateDeniedReasonV0 {
    match error {
        EnvelopeError::WrongClockEpoch
        | EnvelopeError::Invalid(InvalidReasonV0::WrongClockEpoch) => {
            GateDeniedReasonV0::WrongClockEpoch
        }
        EnvelopeError::WrongSourceEpoch
        | EnvelopeError::Invalid(InvalidReasonV0::WrongSourceEpoch) => {
            GateDeniedReasonV0::WrongSourceEpoch
        }
        EnvelopeError::Stale | EnvelopeError::Invalid(InvalidReasonV0::Expired) => {
            GateDeniedReasonV0::StaleSetpoint
        }
        EnvelopeError::Invalid(InvalidReasonV0::NonFinite) => GateDeniedReasonV0::NonFinite,
        EnvelopeError::Invalid(InvalidReasonV0::OutOfRange) => GateDeniedReasonV0::OutOfRange,
        EnvelopeError::Duplicate | EnvelopeError::OutOfOrder | EnvelopeError::Invalid(_) => {
            GateDeniedReasonV0::InvalidSetpoint
        }
        EnvelopeError::Future => GateDeniedReasonV0::InvalidSetpoint,
    }
}

fn safety_envelope_reason(error: &EnvelopeError) -> GateDeniedReasonV0 {
    match error {
        EnvelopeError::WrongClockEpoch
        | EnvelopeError::Invalid(InvalidReasonV0::WrongClockEpoch) => {
            GateDeniedReasonV0::WrongClockEpoch
        }
        EnvelopeError::WrongSourceEpoch
        | EnvelopeError::Invalid(InvalidReasonV0::WrongSourceEpoch) => {
            GateDeniedReasonV0::WrongSourceEpoch
        }
        EnvelopeError::Stale | EnvelopeError::Invalid(InvalidReasonV0::Expired) => {
            GateDeniedReasonV0::StaleSafetyStatus
        }
        EnvelopeError::Duplicate
        | EnvelopeError::OutOfOrder
        | EnvelopeError::Invalid(_)
        | EnvelopeError::Future => GateDeniedReasonV0::InvalidSafetyStatus,
    }
}

fn apply_proposal_fault(
    proposal: &mut ControlProposalV0,
    fault: Option<&ProposalFaultV0>,
    policy: &GatePolicyV0,
) {
    match fault {
        None | Some(ProposalFaultV0::TaskError) => {}
        Some(ProposalFaultV0::Expired) => {
            proposal.valid_until_ns = proposal.captured_at_ns;
        }
        Some(ProposalFaultV0::SourceSequenceMismatch) => {
            proposal.source_sensor_sequence =
                Sequence(proposal.source_sensor_sequence.0.saturating_add(1));
        }
        Some(ProposalFaultV0::NonFinite) => {
            proposal.command.arm_position_rad[0] = f64::NAN;
        }
        Some(ProposalFaultV0::OutOfRange) => {
            proposal.command.arm_position_rad[0] = policy.limits.arm_position_rad[0][1] + 1.0;
        }
    }
}

fn rate_limit(
    requested: &So101CommandV0,
    baseline: &So101CommandV0,
    policy: &GatePolicyV0,
) -> (So101CommandV0, bool) {
    let mut limited = false;
    let arm_position_rad = std::array::from_fn(|index| {
        let (value, was_limited) = clamp_step(
            requested.arm_position_rad[index],
            baseline.arm_position_rad[index],
            policy.max_arm_step_rad[index],
        );
        limited |= was_limited;
        value
    });
    let (gripper_position, gripper_was_limited) = clamp_step(
        requested.gripper_position,
        baseline.gripper_position,
        policy.max_gripper_step,
    );
    limited |= gripper_was_limited;
    (
        So101CommandV0 {
            arm_position_rad,
            gripper_position,
        },
        limited,
    )
}

fn clamp_step(requested: f64, baseline: f64, maximum_step: f64) -> (f64, bool) {
    let minimum = baseline - maximum_step;
    let maximum = baseline + maximum_step;
    (
        requested.clamp(minimum, maximum),
        requested < minimum || requested > maximum,
    )
}

fn denied_output(
    estimate: Option<So101StateV0>,
    control: Option<ControlProposalV0>,
    reason: GateDeniedReasonV0,
) -> SubjectOutputV0 {
    SubjectOutputV0 {
        lifecycle: SubjectLifecycleV0::Running,
        estimate,
        control,
        gate: GateDecisionV0::Denied { reason },
    }
}

fn faulted_output(fault: ReferenceSubjectFaultV0) -> SubjectOutputV0 {
    SubjectOutputV0 {
        lifecycle: SubjectLifecycleV0::Faulted {
            reason: fault.reason().to_owned(),
        },
        estimate: None,
        control: None,
        gate: GateDecisionV0::Denied {
            reason: GateDeniedReasonV0::TaskError,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use hefaos_testbench_contracts::{
        EqualityProfileV0, ExpectedRunV0, GatePolicyV0, JointLimitsV0, MockPlantPolicyV0,
        SafetyPolicyV0, SafetyStatusV0, ScenarioV0, Tick,
    };

    use super::*;

    const FRAME_TTL_NS: u64 = 100;

    fn limits() -> JointLimitsV0 {
        JointLimitsV0 {
            arm_position_rad: [[-2.0, 2.0]; ARM_JOINT_COUNT],
            gripper_position: [0.0, 1.0],
        }
    }

    fn scenario() -> ScenarioV0 {
        ScenarioV0 {
            schema_version: CONTRACT_SCHEMA_VERSION,
            contract: CONTRACT_ID.to_owned(),
            profile: PROFILE_ID.to_owned(),
            name: "reference-unit-test".to_owned(),
            model_digest: "unit-test-model".to_owned(),
            clock_epoch: ClockEpoch(11),
            sensor_source_epoch: SourceEpoch(21),
            setpoint_source_epoch: SourceEpoch(31),
            subject_source_epoch: SourceEpoch(31),
            safety_source_epoch: SourceEpoch(41),
            tick_period_ns: DurationNs(10),
            ticks: Tick(10),
            seed: "0".to_owned(),
            initial_state: So101StateV0::zero(),
            setpoints: Vec::new(),
            safety_actions: Vec::new(),
            faults: Vec::new(),
            gate_policy: GatePolicyV0 {
                sensor_max_age_ns: DurationNs(FRAME_TTL_NS),
                safety_max_age_ns: DurationNs(FRAME_TTL_NS),
                setpoint_max_age_ns: DurationNs(FRAME_TTL_NS),
                proposal_ttl_ns: DurationNs(FRAME_TTL_NS),
                max_arm_step_rad: [0.1; ARM_JOINT_COUNT],
                max_gripper_step: 0.2,
                limits: limits(),
            },
            safety_policy: SafetyPolicyV0 {
                intent_max_age_ns: DurationNs(FRAME_TTL_NS),
                heartbeat_timeout_ns: DurationNs(FRAME_TTL_NS),
                max_arm_step_rad: [0.1; ARM_JOINT_COUNT],
                max_gripper_step: 0.2,
                limits: limits(),
            },
            mock_plant_policy: MockPlantPolicyV0 {
                max_arm_velocity_rad_s: [100.0; ARM_JOINT_COUNT],
                max_gripper_velocity_per_s: 100.0,
            },
            equality: EqualityProfileV0 {
                control_absolute_tolerance: 1.0e-9,
                physics_absolute_tolerance: 1.0e-6,
                require_exact_discrete_trace: true,
                require_bit_exact_mock_trace: true,
            },
            expected: ExpectedRunV0 {
                admitted_ticks: None,
                denied_by_reason: BTreeMap::new(),
                authorized_ticks: None,
                suppressed_ticks: Vec::new(),
                terminal_safety_state: None,
                final_state: None,
            },
        }
    }

    fn sensor(sequence: u64, captured_at_ns: u64, valid_until_ns: u64) -> SensorFrameV0 {
        EnvelopeV0 {
            clock_epoch: ClockEpoch(11),
            source_epoch: SourceEpoch(21),
            sequence: Sequence(sequence),
            captured_at_ns: VirtualTimeNs(captured_at_ns),
            valid_until_ns: VirtualTimeNs(valid_until_ns),
            validity: ValidityV0::Valid,
            payload: So101StateV0::zero(),
        }
    }

    fn setpoint(
        sequence: u64,
        captured_at_ns: u64,
        valid_until_ns: u64,
        command: So101CommandV0,
    ) -> SetpointFrameV0 {
        EnvelopeV0 {
            clock_epoch: ClockEpoch(11),
            source_epoch: SourceEpoch(31),
            sequence: Sequence(sequence),
            captured_at_ns: VirtualTimeNs(captured_at_ns),
            valid_until_ns: VirtualTimeNs(valid_until_ns),
            validity: ValidityV0::Valid,
            payload: command,
        }
    }

    fn safety(sequence: u64, captured_at_ns: u64, valid_until_ns: u64) -> SafetyStatusFrameV0 {
        EnvelopeV0 {
            clock_epoch: ClockEpoch(11),
            source_epoch: SourceEpoch(41),
            sequence: Sequence(sequence),
            captured_at_ns: VirtualTimeNs(captured_at_ns),
            valid_until_ns: VirtualTimeNs(valid_until_ns),
            validity: ValidityV0::Valid,
            payload: SafetyStatusV0 {
                controller_state: SafetyControllerStateV0::Armed {
                    permit_epoch: PermitEpoch(51),
                },
                interlocks_clear: true,
            },
        }
    }

    fn input(sequence: u64, now: u64, command: So101CommandV0) -> SubjectInputV0 {
        SubjectInputV0 {
            tick: Tick(sequence),
            time_ns: VirtualTimeNs(now),
            setpoint: Some(setpoint(sequence, now, now + FRAME_TTL_NS, command)),
            sensor: Some(sensor(sequence, now, now + FRAME_TTL_NS)),
            safety_status: Some(safety(sequence, now, now + FRAME_TTL_NS)),
            proposal_fault: None,
        }
    }

    fn denied_reason(output: &SubjectOutputV0) -> Option<GateDeniedReasonV0> {
        match output.gate {
            GateDecisionV0::Denied { reason } => Some(reason),
            GateDecisionV0::NoCommand | GateDecisionV0::Admitted { .. } => None,
        }
    }

    fn assert_command_close(actual: &So101CommandV0, expected: &So101CommandV0) {
        for (actual, expected) in actual
            .arm_position_rad
            .iter()
            .zip(expected.arm_position_rad)
        {
            assert!((actual - expected).abs() <= f64::EPSILON);
        }
        assert!((actual.gripper_position - expected.gripper_position).abs() <= f64::EPSILON);
    }

    #[test]
    fn nominal_turn_admits_a_typed_intent() {
        let mut subject = ReferenceSubject::new();
        subject.reset(&scenario().subject_config()).unwrap();
        let command = So101CommandV0 {
            arm_position_rad: [0.05, -0.05, 0.04, -0.04, 0.03],
            gripper_position: 0.1,
        };

        let output = subject.step(&input(0, 0, command.clone())).unwrap();

        assert_eq!(output.lifecycle, SubjectLifecycleV0::Running);
        assert_eq!(output.estimate, Some(So101StateV0::zero()));
        let GateDecisionV0::Admitted { intent, limited } = output.gate else {
            panic!("expected admitted intent, got {:?}", output.gate);
        };
        assert!(!limited);
        assert_eq!(intent.permit_epoch, PermitEpoch(51));
        assert_command_close(&intent.command, &command);
        assert_eq!(intent.valid_until_ns, VirtualTimeNs(FRAME_TTL_NS));
    }

    #[test]
    fn hard_limits_reject_before_rate_limits_and_rate_limits_are_stateful() {
        let mut subject = ReferenceSubject::new();
        subject.reset(&scenario().subject_config()).unwrap();

        let requested = So101CommandV0 {
            arm_position_rad: [0.5; ARM_JOINT_COUNT],
            gripper_position: 0.8,
        };
        let first = subject.step(&input(0, 0, requested)).unwrap();
        let GateDecisionV0::Admitted { intent, limited } = first.gate else {
            panic!("expected limited admission");
        };
        assert!(limited);
        assert_command_close(
            &intent.command,
            &So101CommandV0 {
                arm_position_rad: [0.1; ARM_JOINT_COUNT],
                gripper_position: 0.2,
            },
        );

        let mut second_input = input(
            1,
            10,
            So101CommandV0 {
                arm_position_rad: [0.5; ARM_JOINT_COUNT],
                gripper_position: 0.8,
            },
        );
        let sensor = second_input.sensor.as_mut().expect("sensor frame");
        sensor.payload.arm_position_rad = [0.1; ARM_JOINT_COUNT];
        sensor.payload.gripper_position = 0.2;
        let second = subject.step(&second_input).unwrap();
        let GateDecisionV0::Admitted { intent, limited } = second.gate else {
            panic!("expected second limited admission");
        };
        assert!(limited);
        assert_command_close(
            &intent.command,
            &So101CommandV0 {
                arm_position_rad: [0.2; ARM_JOINT_COUNT],
                gripper_position: 0.4,
            },
        );

        let out_of_range = subject
            .step(&input(
                2,
                20,
                So101CommandV0 {
                    arm_position_rad: [3.0, 0.0, 0.0, 0.0, 0.0],
                    gripper_position: 0.5,
                },
            ))
            .unwrap();
        assert_eq!(
            denied_reason(&out_of_range),
            Some(GateDeniedReasonV0::OutOfRange)
        );
    }

    #[test]
    fn half_open_ttl_wrong_epoch_and_duplicate_are_rejected() {
        let command = So101CommandV0::zero();

        let mut stale_subject = ReferenceSubject::new();
        stale_subject.reset(&scenario().subject_config()).unwrap();
        let mut stale = input(0, FRAME_TTL_NS, command.clone());
        stale.sensor = Some(sensor(0, 0, FRAME_TTL_NS));
        assert_eq!(
            denied_reason(&stale_subject.step(&stale).unwrap()),
            Some(GateDeniedReasonV0::StaleSensor)
        );

        let mut epoch_subject = ReferenceSubject::new();
        epoch_subject.reset(&scenario().subject_config()).unwrap();
        let mut wrong_epoch = input(0, 0, command.clone());
        wrong_epoch.sensor.as_mut().unwrap().clock_epoch = ClockEpoch(12);
        assert_eq!(
            denied_reason(&epoch_subject.step(&wrong_epoch).unwrap()),
            Some(GateDeniedReasonV0::WrongClockEpoch)
        );

        let mut source_subject = ReferenceSubject::new();
        source_subject.reset(&scenario().subject_config()).unwrap();
        let mut wrong_source = input(0, 0, command.clone());
        wrong_source.sensor.as_mut().unwrap().source_epoch = SourceEpoch(22);
        assert_eq!(
            denied_reason(&source_subject.step(&wrong_source).unwrap()),
            Some(GateDeniedReasonV0::WrongSourceEpoch)
        );

        let mut duplicate_subject = ReferenceSubject::new();
        duplicate_subject
            .reset(&scenario().subject_config())
            .unwrap();
        duplicate_subject
            .step(&input(0, 0, command.clone()))
            .unwrap();
        let mut duplicate = input(1, 10, command);
        duplicate.sensor.as_mut().unwrap().sequence = Sequence(0);
        assert_eq!(
            denied_reason(&duplicate_subject.step(&duplicate).unwrap()),
            Some(GateDeniedReasonV0::DuplicateSensor)
        );

        let mut gap_subject = ReferenceSubject::new();
        gap_subject.reset(&scenario().subject_config()).unwrap();
        gap_subject
            .step(&input(0, 0, So101CommandV0::zero()))
            .unwrap();
        assert!(matches!(
            gap_subject
                .step(&input(2, 20, So101CommandV0::zero()))
                .unwrap()
                .gate,
            GateDecisionV0::Admitted { .. }
        ));
    }

    #[test]
    fn injected_errors_fail_closed_and_task_errors_latch_until_reset() {
        let mut subject = ReferenceSubject::new();
        subject.reset(&scenario().subject_config()).unwrap();

        let faults = [
            (ProposalFaultV0::NonFinite, GateDeniedReasonV0::NonFinite),
            (ProposalFaultV0::OutOfRange, GateDeniedReasonV0::OutOfRange),
            (
                ProposalFaultV0::Expired,
                GateDeniedReasonV0::ProposalExpired,
            ),
            (
                ProposalFaultV0::SourceSequenceMismatch,
                GateDeniedReasonV0::ProposalSourceMismatch,
            ),
        ];
        for (sequence, (fault, expected)) in faults.into_iter().enumerate() {
            let sequence = u64::try_from(sequence).unwrap();
            let mut faulted = input(sequence, sequence * 10, So101CommandV0::zero());
            faulted.proposal_fault = Some(fault);
            let output = subject.step(&faulted).unwrap();
            assert_eq!(denied_reason(&output), Some(expected));
            assert!(matches!(output.gate, GateDecisionV0::Denied { .. }));
        }

        let mut task_error = input(4, 40, So101CommandV0::zero());
        task_error.proposal_fault = Some(ProposalFaultV0::TaskError);
        let output = subject.step(&task_error).unwrap();
        assert!(matches!(
            output.lifecycle,
            SubjectLifecycleV0::Faulted { .. }
        ));
        assert_eq!(denied_reason(&output), Some(GateDeniedReasonV0::TaskError));

        let output = subject.step(&input(5, 50, So101CommandV0::zero())).unwrap();
        assert!(matches!(
            output.lifecycle,
            SubjectLifecycleV0::Faulted { .. }
        ));
        assert_eq!(denied_reason(&output), Some(GateDeniedReasonV0::TaskError));

        subject.reset(&scenario().subject_config()).unwrap();
        assert!(matches!(
            subject
                .step(&input(0, 0, So101CommandV0::zero()))
                .unwrap()
                .gate,
            GateDecisionV0::Admitted { .. }
        ));
    }

    #[test]
    fn snapshot_restore_resumes_without_input_history() {
        let mut original = ReferenceSubject::new();
        original.reset(&scenario().subject_config()).unwrap();
        original.step(&input(0, 0, So101CommandV0::zero())).unwrap();

        let snapshot = original.snapshot();
        assert!(snapshot.config.is_some());
        assert_eq!(snapshot.sensor_cursor, Some(Sequence(0)));
        assert_eq!(snapshot.setpoint_cursor, Some(Sequence(0)));
        assert_eq!(snapshot.safety_cursor, Some(Sequence(0)));
        assert_eq!(snapshot.next_proposal_sequence, 1);
        assert_eq!(snapshot.active_permit_epoch, Some(PermitEpoch(51)));
        assert_eq!(snapshot.fault_latch, None);

        let serialized = serde_json::to_vec(&snapshot).unwrap();
        let snapshot: ReferenceSubjectSnapshotV0 = serde_json::from_slice(&serialized).unwrap();

        let mut restored = ReferenceSubject::new();
        restored.restore(&snapshot).unwrap();

        let mut continuation = input(1, 10, So101CommandV0::zero());
        continuation
            .safety_status
            .as_mut()
            .unwrap()
            .payload
            .controller_state = SafetyControllerStateV0::Armed {
            permit_epoch: PermitEpoch(52),
        };
        let expected = original.step(&continuation).unwrap();
        let actual = restored.step(&continuation).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            denied_reason(&actual),
            Some(GateDeniedReasonV0::PermitEpochMismatch)
        );
    }

    #[test]
    fn snapshot_restore_preserves_bounded_fault_latch() {
        let mut original = ReferenceSubject::new();
        original.reset(&scenario().subject_config()).unwrap();
        let mut fault = input(0, 0, So101CommandV0::zero());
        fault.proposal_fault = Some(ProposalFaultV0::TaskError);
        original.step(&fault).unwrap();

        let snapshot = original.snapshot();
        assert_eq!(
            snapshot.fault_latch,
            Some(ReferenceSubjectFaultV0::ControllerTaskError)
        );

        let mut restored = ReferenceSubject::new();
        restored.restore(&snapshot).unwrap();
        assert_eq!(
            restored
                .step(&input(1, 10, So101CommandV0::zero()))
                .unwrap(),
            original
                .step(&input(1, 10, So101CommandV0::zero()))
                .unwrap()
        );
    }

    #[test]
    fn invalid_snapshot_is_rejected_without_changing_subject() {
        let mut subject = ReferenceSubject::new();
        subject.reset(&scenario().subject_config()).unwrap();
        subject.step(&input(0, 0, So101CommandV0::zero())).unwrap();
        let before = subject.snapshot();

        let mut invalid_policy = before.clone();
        invalid_policy
            .config
            .as_mut()
            .unwrap()
            .policy
            .proposal_ttl_ns = DurationNs(0);
        assert!(matches!(
            subject.restore(&invalid_policy),
            Err(ReferenceSubjectError::InvalidSnapshot(_))
        ));
        assert_eq!(subject.snapshot(), before);

        let invalid_unconfigured = ReferenceSubjectSnapshotV0 {
            config: None,
            sensor_cursor: Some(Sequence(0)),
            setpoint_cursor: None,
            safety_cursor: None,
            next_proposal_sequence: 0,
            active_permit_epoch: None,
            fault_latch: None,
        };
        assert!(matches!(
            subject.restore(&invalid_unconfigured),
            Err(ReferenceSubjectError::InvalidSnapshot(_))
        ));
        assert_eq!(subject.snapshot(), before);
    }

    #[test]
    fn safety_state_and_permit_changes_require_a_non_armed_transition() {
        let mut subject = ReferenceSubject::new();
        subject.reset(&scenario().subject_config()).unwrap();

        let mut disarmed = input(0, 0, So101CommandV0::zero());
        disarmed
            .safety_status
            .as_mut()
            .unwrap()
            .payload
            .controller_state = SafetyControllerStateV0::Disarmed;
        assert_eq!(
            denied_reason(&subject.step(&disarmed).unwrap()),
            Some(GateDeniedReasonV0::SafetyNotArmed)
        );

        assert!(matches!(
            subject
                .step(&input(1, 10, So101CommandV0::zero()))
                .unwrap()
                .gate,
            GateDecisionV0::Admitted { .. }
        ));

        for sequence in [2, 3] {
            let mut changed_permit = input(sequence, sequence * 10, So101CommandV0::zero());
            changed_permit
                .safety_status
                .as_mut()
                .unwrap()
                .payload
                .controller_state = SafetyControllerStateV0::Armed {
                permit_epoch: PermitEpoch(52),
            };
            assert_eq!(
                denied_reason(&subject.step(&changed_permit).unwrap()),
                Some(GateDeniedReasonV0::PermitEpochMismatch)
            );
        }

        let mut second_disarmed = input(4, 40, So101CommandV0::zero());
        second_disarmed
            .safety_status
            .as_mut()
            .unwrap()
            .payload
            .controller_state = SafetyControllerStateV0::Disarmed;
        assert_eq!(
            denied_reason(&subject.step(&second_disarmed).unwrap()),
            Some(GateDeniedReasonV0::SafetyNotArmed)
        );

        let mut rearmed = input(5, 50, So101CommandV0::zero());
        rearmed
            .safety_status
            .as_mut()
            .unwrap()
            .payload
            .controller_state = SafetyControllerStateV0::Armed {
            permit_epoch: PermitEpoch(52),
        };
        assert!(matches!(
            subject.step(&rearmed).unwrap().gate,
            GateDecisionV0::Admitted { .. }
        ));
    }

    #[test]
    fn harness_subject_trait_uses_the_same_deterministic_boundary() {
        let mut subject = ReferenceSubject::new();
        <ReferenceSubject as Subject>::reset(&mut subject, &scenario().subject_config()).unwrap();
        let output =
            <ReferenceSubject as Subject>::step(&mut subject, &input(0, 0, So101CommandV0::zero()))
                .unwrap();

        assert_eq!(
            <ReferenceSubject as Subject>::id(&subject),
            REFERENCE_SUBJECT_ID
        );
        assert!(matches!(output.gate, GateDecisionV0::Admitted { .. }));
    }
}
