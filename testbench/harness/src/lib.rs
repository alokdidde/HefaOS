//! Deterministic orchestration and verdict logic for the `HefaOS` test bench.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

use hefaos_testbench_contracts::{
    ActuatorIntentV0, AppliedActuationV0, BenchmarkReportV0, CONTRACT_ID, CONTRACT_SCHEMA_VERSION,
    ClockEpoch, DurationNs, EnvelopeV0, ExpectedSafetyStateV0, FaultKindV0, GateDecisionV0,
    GateDeniedReasonV0, GatePolicyV0, JointLimitsV0, LatencySummaryV0, PINNED_SO101_MODEL_DIGEST,
    PROFILE_ID, PermitEpoch, ProposalFaultV0, SO101_TICK_PERIOD_NS, SafetyActionV0,
    SafetyControllerStateV0, SafetyDispositionV0, SafetyPolicyV0, SafetyStatusFrameV0,
    SafetyStatusV0, SafetyTripReasonV0, ScenarioV0, SemanticTraceV0, SensorFrameV0, Sequence,
    SetpointFrameV0, So101CommandV0, So101StateV0, SourceEpoch, SubjectConfigV0, SubjectInputV0,
    SubjectLifecycleV0, SubjectOutputV0, SuppressionReasonV0, Tick, TraceRecordV0, ValidityV0,
    VirtualTimeNs,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

const MAX_SCENARIO_TICKS: u64 = 1_000_000;

#[derive(Debug, Error)]
pub enum SubjectError {
    #[error("subject rejected reset: {0}")]
    Reset(String),
    #[error("subject turn failed: {0}")]
    Step(String),
}

#[derive(Debug, Error)]
pub enum PlantError {
    #[error("invalid initial state: {0}")]
    InvalidInitialState(String),
    #[error("invalid applied actuation: {0}")]
    InvalidActuation(String),
    #[error("plant backend failure: {0}")]
    Backend(String),
}

#[derive(Debug, Error)]
pub enum HarnessError {
    #[error("scenario is invalid:\n{0}")]
    InvalidScenario(String),
    #[error(transparent)]
    Subject(#[from] SubjectError),
    #[error(transparent)]
    Plant(#[from] PlantError),
    #[error("failed to encode evidence: {0}")]
    Evidence(#[from] serde_json::Error),
}

/// Experimental in-process adapter for a graph under test.
///
/// The versioned scenario and trace schemas are durable. This trait may be
/// replaced by subprocess/log normalization if Copper cannot expose a safe
/// one-turn adapter.
pub trait Subject {
    fn id(&self) -> &'static str;
    /// Restores the subject to the supplied deterministic runtime state.
    ///
    /// # Errors
    ///
    /// Returns [`SubjectError`] when the subject cannot accept the configuration.
    fn reset(&mut self, config: &SubjectConfigV0) -> Result<(), SubjectError>;
    /// Executes exactly one virtual-time control turn.
    ///
    /// # Errors
    ///
    /// Returns [`SubjectError`] when the turn cannot produce typed evidence.
    fn step(&mut self, input: &SubjectInputV0) -> Result<SubjectOutputV0, SubjectError>;
}

/// Plant boundary. `apply` stores actuation for the following interval;
/// `advance` consumes it. If `apply` is not called, the plant must suppress
/// active motion for that interval rather than reusing an old command.
pub trait Plant {
    fn id(&self) -> &'static str;
    fn model_digest(&self) -> &'static str;
    /// Restores the plant state before a run.
    ///
    /// # Errors
    ///
    /// Returns [`PlantError`] when the requested state is invalid or cannot be applied.
    fn reset(&mut self, initial: &So101StateV0) -> Result<(), PlantError>;
    /// Captures the state visible to the subject at this virtual turn.
    ///
    /// # Errors
    ///
    /// Returns [`PlantError`] when the backend cannot produce a valid observation.
    fn observe(
        &self,
        tick: Tick,
        now: VirtualTimeNs,
        clock_epoch: ClockEpoch,
        source_epoch: SourceEpoch,
        sequence: Sequence,
    ) -> Result<SensorFrameV0, PlantError>;
    /// Stores a safety-authorized actuation for the next interval.
    ///
    /// # Errors
    ///
    /// Returns [`PlantError`] if the authorized value cannot be represented safely.
    fn apply(&mut self, actuation: &AppliedActuationV0) -> Result<(), PlantError>;
    /// Advances the plant by one requested virtual interval.
    ///
    /// # Errors
    ///
    /// Returns [`PlantError`] if the backend cannot advance by that interval.
    fn advance(&mut self, duration: DurationNs) -> Result<(), PlantError>;
    /// Returns the complete state after the most recent advance.
    ///
    /// # Errors
    ///
    /// Returns [`PlantError`] when the backend state is unavailable or invalid.
    fn state(&self) -> Result<So101StateV0, PlantError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualClock {
    epoch: ClockEpoch,
    now: VirtualTimeNs,
    tick: Tick,
}

impl VirtualClock {
    #[must_use]
    pub const fn new(epoch: ClockEpoch) -> Self {
        Self {
            epoch,
            now: VirtualTimeNs(0),
            tick: Tick(0),
        }
    }

    #[must_use]
    pub const fn epoch(self) -> ClockEpoch {
        self.epoch
    }

    #[must_use]
    pub const fn now(self) -> VirtualTimeNs {
        self.now
    }

    #[must_use]
    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub fn advance(&mut self, duration: DurationNs) {
        self.now = VirtualTimeNs(self.now.0.saturating_add(duration.0));
        self.tick = Tick(self.tick.0.saturating_add(1));
    }
}

#[derive(Debug, Clone)]
pub struct SafetyControllerSim {
    clock_epoch: ClockEpoch,
    source_epoch: SourceEpoch,
    intent_source_epoch: SourceEpoch,
    policy: SafetyPolicyV0,
    state: SafetyControllerStateV0,
    interlocks_clear: bool,
    permit_counter: u64,
    status_sequence: u64,
    armed_at_tick: Option<Tick>,
    watchdog_deadline_ns: Option<VirtualTimeNs>,
    last_intent_sequence: Option<Sequence>,
}

impl SafetyControllerSim {
    #[must_use]
    pub fn new(scenario: &ScenarioV0) -> Self {
        Self::from_config(&scenario.subject_config(), scenario.safety_policy.clone())
    }

    #[must_use]
    pub fn from_config(config: &SubjectConfigV0, policy: SafetyPolicyV0) -> Self {
        Self {
            clock_epoch: config.clock_epoch,
            source_epoch: config.safety_source_epoch,
            intent_source_epoch: config.subject_source_epoch,
            policy,
            state: SafetyControllerStateV0::Disarmed,
            interlocks_clear: true,
            permit_counter: 0,
            status_sequence: 0,
            armed_at_tick: None,
            watchdog_deadline_ns: None,
            last_intent_sequence: None,
        }
    }

    #[must_use]
    pub const fn state(&self) -> &SafetyControllerStateV0 {
        &self.state
    }

    pub fn apply_action(&mut self, tick: Tick, now: VirtualTimeNs, action: &SafetyActionV0) {
        match action {
            SafetyActionV0::ClearFaults => {
                self.interlocks_clear = true;
            }
            SafetyActionV0::Reset => {
                if self.interlocks_clear {
                    self.state = SafetyControllerStateV0::Disarmed;
                    self.armed_at_tick = None;
                    self.watchdog_deadline_ns = None;
                    self.last_intent_sequence = None;
                }
            }
            SafetyActionV0::Arm => {
                if self.interlocks_clear && matches!(self.state, SafetyControllerStateV0::Disarmed)
                {
                    self.permit_counter = self.permit_counter.saturating_add(1);
                    self.state = SafetyControllerStateV0::Armed {
                        permit_epoch: PermitEpoch(self.permit_counter),
                    };
                    self.armed_at_tick = Some(tick);
                    self.watchdog_deadline_ns = Some(VirtualTimeNs(
                        now.0.saturating_add(self.policy.heartbeat_timeout_ns.0),
                    ));
                    self.last_intent_sequence = None;
                }
            }
            SafetyActionV0::Disarm => {
                self.disarm();
            }
        }
    }

    pub fn trip(&mut self, reason: SafetyTripReasonV0) {
        self.interlocks_clear = false;
        if !matches!(self.state, SafetyControllerStateV0::Tripped { .. }) {
            self.state = SafetyControllerStateV0::Tripped { reason };
        }
        self.armed_at_tick = None;
        self.watchdog_deadline_ns = None;
    }

    pub fn disarm(&mut self) {
        if !matches!(self.state, SafetyControllerStateV0::Tripped { .. }) {
            self.state = SafetyControllerStateV0::Disarmed;
            self.armed_at_tick = None;
            self.watchdog_deadline_ns = None;
            self.last_intent_sequence = None;
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> SafetyStatusV0 {
        SafetyStatusV0 {
            controller_state: self.state.clone(),
            interlocks_clear: self.interlocks_clear,
        }
    }

    pub fn status(&mut self, now: VirtualTimeNs, valid_for: DurationNs) -> SafetyStatusFrameV0 {
        let frame = EnvelopeV0 {
            clock_epoch: self.clock_epoch,
            source_epoch: self.source_epoch,
            sequence: Sequence(self.status_sequence),
            captured_at_ns: now,
            valid_until_ns: VirtualTimeNs(now.0.saturating_add(valid_for.0)),
            validity: ValidityV0::Valid,
            payload: self.snapshot(),
        };
        self.status_sequence = self.status_sequence.saturating_add(1);
        frame
    }

    pub fn evaluate(
        &mut self,
        tick: Tick,
        now: VirtualTimeNs,
        intent: Option<&ActuatorIntentV0>,
        observation: &So101StateV0,
    ) -> SafetyDispositionV0 {
        let permit_epoch = match &self.state {
            SafetyControllerStateV0::Disarmed => {
                return SafetyDispositionV0::Suppressed {
                    reason: SuppressionReasonV0::Disarmed,
                };
            }
            SafetyControllerStateV0::Tripped { .. } => {
                return SafetyDispositionV0::Suppressed {
                    reason: SuppressionReasonV0::Tripped,
                };
            }
            SafetyControllerStateV0::Armed { permit_epoch } => *permit_epoch,
        };

        if !self.policy.limits.contains_state(observation) {
            self.trip(SafetyTripReasonV0::FeedbackInvalid);
            return SafetyDispositionV0::Suppressed {
                reason: SuppressionReasonV0::FeedbackInvalid,
            };
        }

        if self.armed_at_tick == Some(tick) {
            return SafetyDispositionV0::Suppressed {
                reason: SuppressionReasonV0::ArmTransition,
            };
        }

        if self
            .watchdog_deadline_ns
            .is_some_and(|deadline| now.0 >= deadline.0)
        {
            self.trip(SafetyTripReasonV0::HeartbeatExpired);
            return SafetyDispositionV0::Suppressed {
                reason: SuppressionReasonV0::Tripped,
            };
        }

        let Some(intent) = intent else {
            return SafetyDispositionV0::Suppressed {
                reason: SuppressionReasonV0::MissingIntent,
            };
        };

        let suppression = if intent.clock_epoch != self.clock_epoch {
            Some(SuppressionReasonV0::WrongClockEpoch)
        } else if intent.source_epoch != self.intent_source_epoch {
            Some(SuppressionReasonV0::WrongSourceEpoch)
        } else if intent.permit_epoch != permit_epoch {
            Some(SuppressionReasonV0::WrongPermitEpoch)
        } else if intent.captured_at_ns.0 > now.0
            || now.0 >= intent.valid_until_ns.0
            || now.0.saturating_sub(intent.captured_at_ns.0) >= self.policy.intent_max_age_ns.0
        {
            Some(SuppressionReasonV0::ExpiredIntent)
        } else if !intent.command.is_finite() {
            Some(SuppressionReasonV0::NonFinite)
        } else if !self.policy.limits.contains_command(&intent.command) {
            Some(SuppressionReasonV0::OutOfRange)
        } else if !command_step_is_safe(&intent.command, observation, &self.policy) {
            Some(SuppressionReasonV0::SlewLimitExceeded)
        } else if let Some(previous) = self.last_intent_sequence {
            match intent.sequence.cmp(&previous) {
                Ordering::Equal => Some(SuppressionReasonV0::DuplicateIntent),
                Ordering::Less => Some(SuppressionReasonV0::OutOfOrderIntent),
                Ordering::Greater => None,
            }
        } else {
            None
        };

        if let Some(reason) = suppression {
            if matches!(
                reason,
                SuppressionReasonV0::NonFinite
                    | SuppressionReasonV0::OutOfRange
                    | SuppressionReasonV0::SlewLimitExceeded
            ) {
                self.trip(SafetyTripReasonV0::CommandInvalid);
            }
            return SafetyDispositionV0::Suppressed { reason };
        }

        self.last_intent_sequence = Some(intent.sequence);
        self.watchdog_deadline_ns = Some(VirtualTimeNs(
            now.0.saturating_add(self.policy.heartbeat_timeout_ns.0),
        ));
        SafetyDispositionV0::Authorized {
            actuation: AppliedActuationV0 {
                tick,
                time_ns: now,
                permit_epoch,
                intent_sequence: intent.sequence,
                command: intent.command.clone(),
            },
        }
    }
}

fn command_step_is_safe(
    command: &So101CommandV0,
    observation: &So101StateV0,
    policy: &SafetyPolicyV0,
) -> bool {
    command
        .arm_position_rad
        .iter()
        .zip(&observation.arm_position_rad)
        .zip(&policy.max_arm_step_rad)
        .all(|((value, previous), maximum)| step_is_within_limit(*value, *previous, *maximum))
        && step_is_within_limit(
            command.gripper_position,
            observation.gripper_position,
            policy.max_gripper_step,
        )
}

fn step_is_within_limit(value: f64, previous: f64, maximum: f64) -> bool {
    let delta = (value - previous).abs();
    delta <= maximum
        || (maximum > 0.0 && delta - maximum <= f64::EPSILON * 8.0 * maximum.abs().max(1.0))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub passed: bool,
    pub failures: Vec<String>,
}

#[derive(Debug)]
pub struct RunOutcome {
    pub trace: SemanticTraceV0,
    pub verdict: Verdict,
    pub control_turn_latency_ns: Vec<u64>,
}

pub struct Runner<S, P> {
    subject: S,
    plant: P,
}

impl<S: Subject, P: Plant> Runner<S, P> {
    #[must_use]
    pub const fn new(subject: S, plant: P) -> Self {
        Self { subject, plant }
    }

    /// Returns the subject after a completed run so subject-owned evidence can
    /// be explicitly finalized by an experimental backend.
    #[must_use]
    pub fn into_subject(self) -> S {
        self.subject
    }

    /// Runs one complete scenario and returns semantic evidence plus its verdict.
    ///
    /// # Errors
    ///
    /// Returns [`HarnessError`] for an invalid scenario, mismatched model,
    /// subject failure at reset, plant failure, or evidence encoding failure.
    #[allow(clippy::too_many_lines)]
    pub fn run(
        &mut self,
        scenario: &ScenarioV0,
        scenario_sha256: &str,
    ) -> Result<RunOutcome, HarnessError> {
        validate_scenario(scenario)
            .map_err(|failures| HarnessError::InvalidScenario(failures.join("\n")))?;

        if self.plant.model_digest() != scenario.model_digest {
            return Err(HarnessError::InvalidScenario(format!(
                "scenario model digest {} does not match plant model digest {}",
                scenario.model_digest,
                self.plant.model_digest()
            )));
        }

        let subject_config = scenario.subject_config();
        self.subject.reset(&subject_config)?;
        self.plant.reset(&scenario.initial_state)?;

        let mut clock = VirtualClock::new(scenario.clock_epoch);
        let mut safety = SafetyControllerSim::new(scenario);
        let capacity = usize::try_from(scenario.ticks.0).map_err(|_| {
            HarnessError::InvalidScenario(
                "ticks do not fit this platform's address space".to_owned(),
            )
        })?;
        let mut records = Vec::with_capacity(capacity);
        let mut latencies = Vec::with_capacity(capacity);
        let mut replayable = true;

        for _ in 0..scenario.ticks.0 {
            let tick = clock.tick();
            let now = clock.now();

            let safety_actions: Vec<_> = scenario
                .safety_actions
                .iter()
                .filter(|scheduled| scheduled.at_tick == tick)
                .map(|scheduled| scheduled.action.clone())
                .collect();
            for action in &safety_actions {
                safety.apply_action(tick, now, action);
            }

            let active_faults: Vec<_> = scenario
                .faults
                .iter()
                .filter(|scheduled| scheduled.active_at(tick))
                .map(|scheduled| scheduled.fault.clone())
                .collect();

            for fault in &active_faults {
                match fault {
                    FaultKindV0::EmergencyStop => safety.trip(SafetyTripReasonV0::EmergencyStop),
                    FaultKindV0::DriveFault => safety.trip(SafetyTripReasonV0::DriveFault),
                    FaultKindV0::RevokePermit => safety.disarm(),
                    _ => {}
                }
            }

            let mut sensor = self.plant.observe(
                tick,
                now,
                scenario.clock_epoch,
                scenario.sensor_source_epoch,
                Sequence(tick.0),
            )?;
            let safety_observation = self.plant.state()?;
            apply_sensor_faults(&mut sensor, &active_faults, scenario);
            let sensor = if active_faults
                .iter()
                .any(|fault| matches!(fault, FaultKindV0::DropSensor))
            {
                None
            } else {
                Some(sensor)
            };

            let mut safety_status = safety.status(now, scenario.tick_period_ns);
            apply_safety_status_faults(&mut safety_status, &active_faults, scenario);
            let safety_status = if active_faults
                .iter()
                .any(|fault| matches!(fault, FaultKindV0::DropSafetyStatus))
            {
                None
            } else {
                Some(safety_status)
            };

            let mut setpoint = current_setpoint(scenario, tick);
            if let Some(frame) = &mut setpoint {
                apply_setpoint_faults(frame, &active_faults, scenario);
            }
            let setpoint = if active_faults
                .iter()
                .any(|fault| matches!(fault, FaultKindV0::DropSetpoint))
            {
                None
            } else {
                setpoint
            };
            let proposal_fault = active_faults.iter().find_map(|fault| match fault {
                FaultKindV0::Proposal { fault } => Some(fault.clone()),
                _ => None,
            });
            let input = SubjectInputV0 {
                tick,
                time_ns: now,
                setpoint,
                sensor: sensor.clone(),
                safety_status: safety_status.clone(),
                proposal_fault,
            };

            let started = Instant::now();
            let subject_result = catch_unwind(AssertUnwindSafe(|| self.subject.step(&input)));
            let elapsed_ns = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
            latencies.push(elapsed_ns);

            let subject_output = match subject_result {
                Ok(Ok(output)) => output,
                Ok(Err(error)) => {
                    replayable = false;
                    safety.trip(SafetyTripReasonV0::SubjectFault);
                    faulted_output(error.to_string())
                }
                Err(_) => {
                    replayable = false;
                    safety.trip(SafetyTripReasonV0::SubjectFault);
                    faulted_output("subject panicked".to_owned())
                }
            };

            if matches!(subject_output.lifecycle, SubjectLifecycleV0::Faulted { .. }) {
                safety.trip(SafetyTripReasonV0::SubjectFault);
            }

            let drop_intent = active_faults
                .iter()
                .any(|fault| matches!(fault, FaultKindV0::DropIntent));
            let intent = if drop_intent {
                None
            } else {
                match &subject_output.gate {
                    GateDecisionV0::Admitted { intent, .. } => Some(intent),
                    GateDecisionV0::NoCommand | GateDecisionV0::Denied { .. } => None,
                }
            };

            let disposition = safety.evaluate(tick, now, intent, &safety_observation);
            if let SafetyDispositionV0::Authorized { actuation } = &disposition {
                self.plant.apply(actuation)?;
            }
            self.plant.advance(scenario.tick_period_ns)?;
            let plant_state_after = self.plant.state()?;

            records.push(TraceRecordV0 {
                tick,
                time_ns: now,
                safety_actions,
                active_faults,
                safety_observation,
                subject_input: input,
                subject_output,
                safety_disposition: disposition,
                safety_controller_after: safety.snapshot(),
                plant_state_after,
            });
            clock.advance(scenario.tick_period_ns);
        }

        let summary = summarize(&records, safety.state(), replayable);
        let trace = SemanticTraceV0 {
            schema_version: CONTRACT_SCHEMA_VERSION,
            contract: CONTRACT_ID.to_owned(),
            profile: PROFILE_ID.to_owned(),
            scenario_name: scenario.name.clone(),
            scenario_sha256: scenario_sha256.to_owned(),
            model_digest: scenario.model_digest.clone(),
            seed: scenario.seed.clone(),
            tick_period_ns: scenario.tick_period_ns,
            expected_ticks: scenario.ticks,
            subject_config,
            safety_policy: scenario.safety_policy.clone(),
            subject_id: self.subject.id().to_owned(),
            plant_id: self.plant.id().to_owned(),
            equality: scenario.equality.clone(),
            records,
            summary,
        };
        let verdict = evaluate_expectations(scenario, &trace);

        Ok(RunOutcome {
            trace,
            verdict,
            control_turn_latency_ns: latencies,
        })
    }
}

fn faulted_output(reason: String) -> SubjectOutputV0 {
    SubjectOutputV0 {
        lifecycle: SubjectLifecycleV0::Faulted { reason },
        estimate: None,
        control: None,
        gate: GateDecisionV0::Denied {
            reason: GateDeniedReasonV0::TaskError,
        },
    }
}

fn current_setpoint(scenario: &ScenarioV0, tick: Tick) -> Option<SetpointFrameV0> {
    let (_, keyframe) = scenario
        .setpoints
        .iter()
        .enumerate()
        .rev()
        .find(|(_, keyframe)| keyframe.at_tick <= tick)?;
    // The scenario goal is retained, but its source publishes a fresh sample on
    // every control tick. This keeps delivery cursor semantics independent from
    // how often the requested target changes.
    let captured_at = VirtualTimeNs(tick.0.saturating_mul(scenario.tick_period_ns.0));
    Some(EnvelopeV0 {
        clock_epoch: scenario.clock_epoch,
        source_epoch: scenario.setpoint_source_epoch,
        sequence: Sequence(tick.0),
        captured_at_ns: captured_at,
        valid_until_ns: VirtualTimeNs(
            captured_at
                .0
                .saturating_add(scenario.gate_policy.setpoint_max_age_ns.0),
        ),
        validity: ValidityV0::Valid,
        payload: keyframe.command.clone(),
    })
}

fn apply_sensor_faults(sensor: &mut SensorFrameV0, faults: &[FaultKindV0], scenario: &ScenarioV0) {
    for fault in faults {
        match fault {
            FaultKindV0::InvalidateSensor { reason } => {
                sensor.validity = ValidityV0::Invalid(reason.clone());
            }
            FaultKindV0::OutOfRangeSensor => {
                sensor.payload.arm_position_rad[0] =
                    scenario.gate_policy.limits.arm_position_rad[0][1] + 1.0;
            }
            FaultKindV0::FutureSensor { offset_ns } => {
                sensor.captured_at_ns =
                    VirtualTimeNs(sensor.captured_at_ns.0.saturating_add(offset_ns.0));
                sensor.valid_until_ns =
                    VirtualTimeNs(sensor.valid_until_ns.0.saturating_add(offset_ns.0));
            }
            FaultKindV0::OverAgeSensor { age_ns } => {
                sensor.captured_at_ns =
                    VirtualTimeNs(sensor.captured_at_ns.0.saturating_sub(age_ns.0));
            }
            FaultKindV0::StaleSensor { age_ns } => {
                sensor.captured_at_ns =
                    VirtualTimeNs(sensor.captured_at_ns.0.saturating_sub(age_ns.0));
                sensor.valid_until_ns = sensor.captured_at_ns;
            }
            FaultKindV0::DuplicateSensor => {
                sensor.sequence = Sequence(sensor.sequence.0.saturating_sub(1));
            }
            FaultKindV0::ReorderSensor => {
                sensor.sequence = Sequence(sensor.sequence.0.saturating_sub(2));
            }
            FaultKindV0::WrongSensorClockEpoch => {
                sensor.clock_epoch = ClockEpoch(scenario.clock_epoch.0.saturating_add(1));
            }
            FaultKindV0::WrongSensorSourceEpoch => {
                sensor.source_epoch = SourceEpoch(scenario.sensor_source_epoch.0.saturating_add(1));
            }
            _ => {}
        }
    }
}

fn apply_setpoint_faults(
    setpoint: &mut SetpointFrameV0,
    faults: &[FaultKindV0],
    scenario: &ScenarioV0,
) {
    for fault in faults {
        match fault {
            FaultKindV0::InvalidateSetpoint { reason } => {
                setpoint.validity = ValidityV0::Invalid(reason.clone());
            }
            FaultKindV0::OutOfRangeSetpoint => {
                setpoint.payload.arm_position_rad[0] =
                    scenario.gate_policy.limits.arm_position_rad[0][1] + 1.0;
            }
            FaultKindV0::FutureSetpoint { offset_ns } => {
                setpoint.captured_at_ns =
                    VirtualTimeNs(setpoint.captured_at_ns.0.saturating_add(offset_ns.0));
                setpoint.valid_until_ns =
                    VirtualTimeNs(setpoint.valid_until_ns.0.saturating_add(offset_ns.0));
            }
            FaultKindV0::OverAgeSetpoint { age_ns } => {
                setpoint.captured_at_ns =
                    VirtualTimeNs(setpoint.captured_at_ns.0.saturating_sub(age_ns.0));
            }
            FaultKindV0::StaleSetpoint { age_ns } => {
                setpoint.captured_at_ns =
                    VirtualTimeNs(setpoint.captured_at_ns.0.saturating_sub(age_ns.0));
                setpoint.valid_until_ns = setpoint.captured_at_ns;
            }
            FaultKindV0::DuplicateSetpoint => {
                setpoint.sequence = Sequence(setpoint.sequence.0.saturating_sub(1));
            }
            FaultKindV0::ReorderSetpoint => {
                setpoint.sequence = Sequence(setpoint.sequence.0.saturating_sub(2));
            }
            FaultKindV0::WrongSetpointClockEpoch => {
                setpoint.clock_epoch = ClockEpoch(scenario.clock_epoch.0.saturating_add(1));
            }
            FaultKindV0::WrongSetpointSourceEpoch => {
                setpoint.source_epoch =
                    SourceEpoch(scenario.setpoint_source_epoch.0.saturating_add(1));
            }
            _ => {}
        }
    }
}

fn apply_safety_status_faults(
    status: &mut SafetyStatusFrameV0,
    faults: &[FaultKindV0],
    scenario: &ScenarioV0,
) {
    for fault in faults {
        match fault {
            FaultKindV0::InvalidateSafetyStatus { reason } => {
                status.validity = ValidityV0::Invalid(reason.clone());
            }
            FaultKindV0::OpenSafetyInterlock => {
                status.payload.interlocks_clear = false;
            }
            FaultKindV0::FutureSafetyStatus { offset_ns } => {
                status.captured_at_ns =
                    VirtualTimeNs(status.captured_at_ns.0.saturating_add(offset_ns.0));
                status.valid_until_ns =
                    VirtualTimeNs(status.valid_until_ns.0.saturating_add(offset_ns.0));
            }
            FaultKindV0::OverAgeSafetyStatus { age_ns } => {
                status.captured_at_ns =
                    VirtualTimeNs(status.captured_at_ns.0.saturating_sub(age_ns.0));
            }
            FaultKindV0::StaleSafetyStatus { age_ns } => {
                status.captured_at_ns =
                    VirtualTimeNs(status.captured_at_ns.0.saturating_sub(age_ns.0));
                status.valid_until_ns = status.captured_at_ns;
            }
            FaultKindV0::DuplicateSafetyStatus => {
                status.sequence = Sequence(status.sequence.0.saturating_sub(1));
            }
            FaultKindV0::ReorderSafetyStatus => {
                status.sequence = Sequence(status.sequence.0.saturating_sub(2));
            }
            FaultKindV0::WrongSafetyClockEpoch => {
                status.clock_epoch = ClockEpoch(scenario.clock_epoch.0.saturating_add(1));
            }
            FaultKindV0::WrongSafetySourceEpoch => {
                status.source_epoch = SourceEpoch(scenario.safety_source_epoch.0.saturating_add(1));
            }
            FaultKindV0::WrongSafetyPermitEpoch => {
                if let SafetyControllerStateV0::Armed { permit_epoch } =
                    &mut status.payload.controller_state
                {
                    permit_epoch.0 = permit_epoch.0.saturating_add(1);
                }
            }
            _ => {}
        }
    }
}

fn summarize(
    records: &[TraceRecordV0],
    safety_state: &SafetyControllerStateV0,
    replayable: bool,
) -> hefaos_testbench_contracts::RunSummaryV0 {
    let mut admitted_ticks = 0;
    let mut denied_by_reason = BTreeMap::new();
    let mut authorized_ticks = 0;
    let mut suppressed_ticks = Vec::new();

    for record in records {
        match &record.subject_output.gate {
            GateDecisionV0::Admitted { .. } => admitted_ticks += 1,
            GateDecisionV0::Denied { reason } => {
                *denied_by_reason.entry(*reason).or_insert(0) += 1;
            }
            GateDecisionV0::NoCommand => {}
        }
        match record.safety_disposition {
            SafetyDispositionV0::Authorized { .. } => authorized_ticks += 1,
            SafetyDispositionV0::Suppressed { .. } => suppressed_ticks.push(record.tick),
        }
    }

    hefaos_testbench_contracts::RunSummaryV0 {
        admitted_ticks,
        denied_by_reason,
        authorized_ticks,
        suppressed_ticks,
        terminal_safety_state: safety_state_class(safety_state),
        replayable,
    }
}

fn safety_state_class(state: &SafetyControllerStateV0) -> ExpectedSafetyStateV0 {
    match state {
        SafetyControllerStateV0::Disarmed => ExpectedSafetyStateV0::Disarmed,
        SafetyControllerStateV0::Armed { .. } => ExpectedSafetyStateV0::Armed,
        SafetyControllerStateV0::Tripped { .. } => ExpectedSafetyStateV0::Tripped,
    }
}

#[must_use]
pub fn evaluate_expectations(scenario: &ScenarioV0, trace: &SemanticTraceV0) -> Verdict {
    let mut failures = Vec::new();
    if let Err(evidence_failures) = validate_trace_evidence(trace) {
        failures.extend(
            evidence_failures
                .into_iter()
                .map(|failure| format!("invalid trace evidence: {failure}")),
        );
    }
    let expected = &scenario.expected;
    let actual = &trace.summary;

    if let Some(value) = expected.admitted_ticks
        && actual.admitted_ticks != value
    {
        failures.push(format!(
            "expected {value} admitted ticks, got {}",
            actual.admitted_ticks
        ));
    }
    if actual.denied_by_reason != expected.denied_by_reason {
        failures.push(format!(
            "denial counts differ: expected {:?}, got {:?}",
            expected.denied_by_reason, actual.denied_by_reason
        ));
    }
    if let Some(value) = expected.authorized_ticks
        && actual.authorized_ticks != value
    {
        failures.push(format!(
            "expected {value} authorized ticks, got {}",
            actual.authorized_ticks
        ));
    }
    if actual.suppressed_ticks != expected.suppressed_ticks {
        failures.push(format!(
            "suppressed ticks differ: expected {:?}, got {:?}",
            expected.suppressed_ticks, actual.suppressed_ticks
        ));
    }
    if let Some(expected_state) = &expected.terminal_safety_state
        && &actual.terminal_safety_state != expected_state
    {
        failures.push(format!(
            "expected terminal safety state {expected_state:?}, got {:?}",
            actual.terminal_safety_state
        ));
    }
    if let Some(expected_state) = &expected.final_state {
        let Some(actual_state) = trace.records.last().map(|record| &record.plant_state_after)
        else {
            failures.push("trace has no final plant state".to_owned());
            return Verdict {
                passed: false,
                failures,
            };
        };
        compare_state(
            actual_state,
            &expected_state.state,
            expected_state.absolute_tolerance,
            "final state",
            &mut failures,
        );
    }

    Verdict {
        passed: failures.is_empty(),
        failures,
    }
}

/// Validates every static invariant needed for a bounded deterministic run.
///
/// # Errors
///
/// Returns all discovered schema, schedule, policy, or expectation problems.
#[allow(clippy::too_many_lines)]
pub fn validate_scenario(scenario: &ScenarioV0) -> Result<(), Vec<String>> {
    let mut failures = Vec::new();
    if scenario.schema_version != CONTRACT_SCHEMA_VERSION {
        failures.push(format!(
            "unsupported schemaVersion {}",
            scenario.schema_version
        ));
    }
    if scenario.contract != CONTRACT_ID {
        failures.push(format!("unsupported contract {}", scenario.contract));
    }
    if scenario.profile != PROFILE_ID {
        failures.push(format!("unsupported profile {}", scenario.profile));
    }
    if scenario.name.trim().is_empty() || scenario.seed.trim().is_empty() {
        failures.push("scenario name and seed must be non-empty".to_owned());
    }
    if scenario.model_digest != PINNED_SO101_MODEL_DIGEST {
        failures.push(format!(
            "modelDigest must identify the pinned SO-101 model: {PINNED_SO101_MODEL_DIGEST}"
        ));
    }
    if scenario.tick_period_ns.0 != SO101_TICK_PERIOD_NS {
        failures.push(format!(
            "tickPeriodNs must be the pinned {SO101_TICK_PERIOD_NS} ns"
        ));
    }
    if scenario.ticks.0 == 0 || scenario.ticks.0 > MAX_SCENARIO_TICKS {
        failures.push(format!("ticks must be between 1 and {MAX_SCENARIO_TICKS}"));
    }
    let source_epochs = [
        scenario.sensor_source_epoch,
        scenario.setpoint_source_epoch,
        scenario.subject_source_epoch,
        scenario.safety_source_epoch,
    ];
    if source_epochs.iter().any(|epoch| epoch.0 == 0)
        || source_epochs
            .iter()
            .enumerate()
            .any(|(index, epoch)| source_epochs[..index].contains(epoch))
    {
        failures.push("all source epochs must be non-zero and pairwise distinct".to_owned());
    }
    if [
        scenario.gate_policy.sensor_max_age_ns,
        scenario.gate_policy.safety_max_age_ns,
        scenario.gate_policy.setpoint_max_age_ns,
        scenario.gate_policy.proposal_ttl_ns,
        scenario.safety_policy.intent_max_age_ns,
        scenario.safety_policy.heartbeat_timeout_ns,
    ]
    .iter()
    .any(|duration| duration.0 == 0)
    {
        failures
            .push("all ages, TTLs, and the safety heartbeat timeout must be positive".to_owned());
    }
    if !valid_joint_limits(&scenario.gate_policy.limits)
        || !valid_joint_limits(&scenario.safety_policy.limits)
    {
        failures.push("gate and safety limits must contain finite ordered ranges".to_owned());
    }
    if !safety_policy_is_no_looser(&scenario.gate_policy, &scenario.safety_policy) {
        failures.push(
            "safety limits, slew, and intent age must be no looser than the subject gate"
                .to_owned(),
        );
    }
    if !scenario.initial_state.is_finite()
        || !scenario
            .gate_policy
            .limits
            .contains_state(&scenario.initial_state)
        || !scenario
            .safety_policy
            .limits
            .contains_state(&scenario.initial_state)
    {
        failures.push("initialState must be finite and inside gate and safety limits".to_owned());
    }
    if scenario.setpoints.first().map(|keyframe| keyframe.at_tick) != Some(Tick(0)) {
        failures.push("the first setpoint must start at tick 0".to_owned());
    }
    for window in scenario.setpoints.windows(2) {
        if window[0].at_tick >= window[1].at_tick {
            failures.push("setpoint ticks must be strictly increasing".to_owned());
            break;
        }
    }
    for setpoint in &scenario.setpoints {
        if setpoint.at_tick.0 >= scenario.ticks.0 {
            failures.push(format!(
                "setpoint tick {} is outside the run",
                setpoint.at_tick
            ));
        }
        if !scenario
            .gate_policy
            .limits
            .contains_command(&setpoint.command)
        {
            failures.push(format!(
                "setpoint at tick {} is outside limits",
                setpoint.at_tick
            ));
        }
    }
    for fault in &scenario.faults {
        let end = fault.start_tick.0.checked_add(fault.duration_ticks.0);
        if fault.duration_ticks.0 == 0
            || fault.start_tick.0 >= scenario.ticks.0
            || end.is_none_or(|value| value > scenario.ticks.0)
        {
            failures.push(format!(
                "invalid fault schedule at tick {}",
                fault.start_tick
            ));
        }
        if matches!(
            &fault.fault,
            FaultKindV0::FutureSensor { offset_ns }
                | FaultKindV0::FutureSetpoint { offset_ns }
                | FaultKindV0::FutureSafetyStatus { offset_ns }
                if offset_ns.0 == 0
        ) {
            failures.push(format!(
                "future-frame offset at tick {} must be positive",
                fault.start_tick
            ));
        }
        if matches!(
            &fault.fault,
            FaultKindV0::OverAgeSensor { age_ns }
                | FaultKindV0::OverAgeSetpoint { age_ns }
                | FaultKindV0::OverAgeSafetyStatus { age_ns }
                if age_ns.0 == 0
        ) {
            failures.push(format!(
                "over-age frame offset at tick {} must be positive",
                fault.start_tick
            ));
        }
    }
    for action in &scenario.safety_actions {
        if action.at_tick.0 >= scenario.ticks.0 {
            failures.push(format!(
                "safety action tick {} is outside the run",
                action.at_tick
            ));
        }
    }
    if scenario
        .safety_actions
        .windows(2)
        .any(|window| window[0].at_tick > window[1].at_tick)
    {
        failures.push("safety action ticks must be non-decreasing".to_owned());
    }
    for (name, value) in [
        (
            "controlAbsoluteTolerance",
            scenario.equality.control_absolute_tolerance,
        ),
        (
            "physicsAbsoluteTolerance",
            scenario.equality.physics_absolute_tolerance,
        ),
        ("maxGripperStep", scenario.gate_policy.max_gripper_step),
        (
            "safetyMaxGripperStep",
            scenario.safety_policy.max_gripper_step,
        ),
        (
            "maxGripperVelocityPerS",
            scenario.mock_plant_policy.max_gripper_velocity_per_s,
        ),
    ] {
        if !value.is_finite() || value < 0.0 {
            failures.push(format!("{name} must be finite and non-negative"));
        }
    }
    if scenario
        .gate_policy
        .max_arm_step_rad
        .iter()
        .chain(&scenario.safety_policy.max_arm_step_rad)
        .chain(&scenario.mock_plant_policy.max_arm_velocity_rad_s)
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        failures.push("rate and step limits must be finite and non-negative".to_owned());
    }
    if !scenario.equality.require_exact_discrete_trace {
        failures.push("v0 requires exact discrete trace comparison".to_owned());
    }
    if scenario
        .expected
        .suppressed_ticks
        .windows(2)
        .any(|ticks| ticks[0] >= ticks[1])
        || scenario
            .expected
            .suppressed_ticks
            .iter()
            .any(|tick| tick.0 >= scenario.ticks.0)
    {
        failures.push(
            "expected suppressed ticks must be unique, sorted, and inside the run".to_owned(),
        );
    }
    if let Some(authorized) = scenario.expected.authorized_ticks
        && authorized.saturating_add(scenario.expected.suppressed_ticks.len() as u64)
            != scenario.ticks.0
    {
        failures.push("authorizedTicks plus suppressedTicks must equal ticks".to_owned());
    }
    let denied: u64 = scenario.expected.denied_by_reason.values().copied().sum();
    if scenario
        .expected
        .denied_by_reason
        .values()
        .any(|count| *count == 0)
        || denied > scenario.ticks.0
    {
        failures.push("denial counts must be positive and cannot exceed ticks".to_owned());
    }
    if let Some(admitted) = scenario.expected.admitted_ticks
        && admitted.saturating_add(denied) > scenario.ticks.0
    {
        failures.push("admitted and denied counts cannot exceed ticks".to_owned());
    }
    if let Some(expected) = &scenario.expected.final_state
        && (!expected.state.is_finite()
            || !expected.absolute_tolerance.is_finite()
            || expected.absolute_tolerance < 0.0)
    {
        failures
            .push("expected final state and tolerance must be finite and non-negative".to_owned());
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

fn valid_joint_limits(limits: &hefaos_testbench_contracts::JointLimitsV0) -> bool {
    limits
        .arm_position_rad
        .iter()
        .chain(std::iter::once(&limits.gripper_position))
        .all(|[minimum, maximum]| minimum.is_finite() && maximum.is_finite() && minimum <= maximum)
}

fn safety_policy_is_no_looser(gate: &GatePolicyV0, safety: &SafetyPolicyV0) -> bool {
    safety.intent_max_age_ns.0 <= gate.proposal_ttl_ns.0
        && safety.max_gripper_step <= gate.max_gripper_step
        && safety
            .max_arm_step_rad
            .iter()
            .zip(&gate.max_arm_step_rad)
            .all(|(safety_step, gate_step)| safety_step <= gate_step)
        && limits_are_subset(&safety.limits, &gate.limits)
}

fn limits_are_subset(inner: &JointLimitsV0, outer: &JointLimitsV0) -> bool {
    inner
        .arm_position_rad
        .iter()
        .zip(&outer.arm_position_rad)
        .all(|([inner_min, inner_max], [outer_min, outer_max])| {
            inner_min >= outer_min && inner_max <= outer_max
        })
        && inner.gripper_position[0] >= outer.gripper_position[0]
        && inner.gripper_position[1] <= outer.gripper_position[1]
}

/// Checks that a semantic trace is complete, finite, internally consistent,
/// and safe to use as replay or comparison evidence.
///
/// # Errors
///
/// Returns every discovered evidence-integrity or safety-invariant failure.
#[allow(clippy::too_many_lines)]
pub fn validate_trace_evidence(trace: &SemanticTraceV0) -> Result<(), Vec<String>> {
    let mut failures = Vec::new();
    if trace.schema_version != CONTRACT_SCHEMA_VERSION
        || trace.contract != CONTRACT_ID
        || trace.profile != PROFILE_ID
    {
        failures.push("trace schema, contract, or profile is unsupported".to_owned());
    }
    if trace.scenario_name.trim().is_empty()
        || trace.seed.trim().is_empty()
        || trace.subject_id.trim().is_empty()
        || trace.plant_id.trim().is_empty()
    {
        failures.push("trace provenance fields must be non-empty".to_owned());
    }
    if trace.scenario_sha256.len() != 64
        || !trace
            .scenario_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        failures.push("scenarioSha256 must be 64 lowercase hexadecimal characters".to_owned());
    }
    if trace.model_digest != PINNED_SO101_MODEL_DIGEST {
        failures.push("trace model digest does not identify the pinned SO-101 model".to_owned());
    }
    if trace.tick_period_ns.0 != SO101_TICK_PERIOD_NS
        || trace.subject_config.tick_period_ns != trace.tick_period_ns
    {
        failures.push("trace tick period is inconsistent with the pinned profile".to_owned());
    }
    if trace.subject_config.schema_version != trace.schema_version
        || trace.subject_config.contract != trace.contract
        || trace.subject_config.profile != trace.profile
    {
        failures.push("subject configuration identity differs from trace identity".to_owned());
    }
    if !valid_joint_limits(&trace.subject_config.gate_policy.limits)
        || !valid_joint_limits(&trace.safety_policy.limits)
        || trace.safety_policy.intent_max_age_ns.0 == 0
        || trace.safety_policy.heartbeat_timeout_ns.0 == 0
        || trace
            .safety_policy
            .max_arm_step_rad
            .iter()
            .any(|step| !step.is_finite() || *step < 0.0)
        || !trace.safety_policy.max_gripper_step.is_finite()
        || trace.safety_policy.max_gripper_step < 0.0
    {
        failures.push("trace contains an invalid gate or safety policy".to_owned());
    }
    if !safety_policy_is_no_looser(&trace.subject_config.gate_policy, &trace.safety_policy) {
        failures.push("trace safety policy is looser than the subject gate policy".to_owned());
    }
    if !trace
        .subject_config
        .gate_policy
        .limits
        .contains_state(&trace.subject_config.initial_state)
        || !trace
            .safety_policy
            .limits
            .contains_state(&trace.subject_config.initial_state)
    {
        failures.push("trace initial state is outside gate or safety limits".to_owned());
    }
    if !trace.equality.control_absolute_tolerance.is_finite()
        || trace.equality.control_absolute_tolerance < 0.0
        || !trace.equality.physics_absolute_tolerance.is_finite()
        || trace.equality.physics_absolute_tolerance < 0.0
        || !trace.equality.require_exact_discrete_trace
    {
        failures.push("trace contains an invalid equality profile".to_owned());
    }
    let record_count = u64::try_from(trace.records.len()).unwrap_or(u64::MAX);
    if trace.expected_ticks.0 == 0 || record_count != trace.expected_ticks.0 {
        failures.push(format!(
            "expected {} trace records, found {record_count}",
            trace.expected_ticks
        ));
    }
    if !trace.summary.replayable {
        failures.push("trace is marked non-replayable".to_owned());
    }

    let mut task_fault_latched = false;
    let mut task_fault_owns_trip = false;
    for (index, record) in trace.records.iter().enumerate() {
        let expected_tick = u64::try_from(index).unwrap_or(u64::MAX);
        let expected_time = expected_tick.saturating_mul(trace.tick_period_ns.0);
        if record.tick.0 != expected_tick || record.time_ns.0 != expected_time {
            failures.push(format!(
                "record {index} is not on the declared virtual timeline"
            ));
        }
        if record.subject_input.tick != record.tick
            || record.subject_input.time_ns != record.time_ns
        {
            failures.push(format!(
                "record {index} input tick/time does not match the record"
            ));
        }
        let expected_proposal_fault = record.active_faults.iter().find_map(|fault| match fault {
            FaultKindV0::Proposal { fault } => Some(fault.clone()),
            _ => None,
        });
        if record.subject_input.proposal_fault.as_ref() != expected_proposal_fault.as_ref() {
            failures.push(format!(
                "record {index} proposal-fault input does not match its scheduled event"
            ));
        }
        if !task_fault_latched
            && matches!(
                expected_proposal_fault.as_ref(),
                Some(ProposalFaultV0::TaskError)
            )
        {
            task_fault_latched = true;
            task_fault_owns_trip = index == 0
                || !matches!(
                    &trace.records[index - 1]
                        .safety_controller_after
                        .controller_state,
                    SafetyControllerStateV0::Tripped { .. }
                );
        }
        if task_fault_latched {
            if !matches!(
                &record.subject_output.lifecycle,
                SubjectLifecycleV0::Faulted { reason } if !reason.trim().is_empty()
            ) || !matches!(
                &record.subject_output.gate,
                GateDecisionV0::Denied {
                    reason: GateDeniedReasonV0::TaskError
                }
            ) {
                failures.push(format!(
                    "record {index} does not preserve the injected task-fault lifecycle"
                ));
            }
            if task_fault_owns_trip
                && (!matches!(
                    &record.safety_controller_after.controller_state,
                    SafetyControllerStateV0::Tripped {
                        reason: SafetyTripReasonV0::SubjectFault
                    }
                ) || !matches!(
                    &record.safety_disposition,
                    SafetyDispositionV0::Suppressed {
                        reason: SuppressionReasonV0::Tripped
                    }
                ))
            {
                failures.push(format!(
                    "record {index} does not preserve the immediate SubjectFault trip"
                ));
            }
        }
        if record
            .subject_input
            .sensor
            .as_ref()
            .is_some_and(|frame| !frame.payload.is_finite())
            || record
                .subject_input
                .setpoint
                .as_ref()
                .is_some_and(|frame| !frame.payload.is_finite())
            || record
                .subject_output
                .estimate
                .as_ref()
                .is_some_and(|state| !state.is_finite())
            || record
                .subject_output
                .control
                .as_ref()
                .is_some_and(|proposal| !proposal.command.is_finite())
            || matches!(
                &record.subject_output.gate,
                GateDecisionV0::Admitted { intent, .. } if !intent.command.is_finite()
            )
            || matches!(
                &record.safety_disposition,
                SafetyDispositionV0::Authorized { actuation } if !actuation.command.is_finite()
            )
            || !record.safety_observation.is_finite()
            || !record.plant_state_after.is_finite()
        {
            failures.push(format!(
                "record {index} contains non-finite numeric evidence"
            ));
        }
        let expected_safety_observation = if index == 0 {
            &trace.subject_config.initial_state
        } else {
            &trace.records[index - 1].plant_state_after
        };
        if &record.safety_observation != expected_safety_observation {
            failures.push(format!(
                "record {index} safety observation is not the preceding authoritative plant state"
            ));
        }

        match &record.safety_disposition {
            SafetyDispositionV0::Authorized { actuation } => {
                let admitted = match &record.subject_output.gate {
                    GateDecisionV0::Admitted { intent, .. } => Some(intent),
                    GateDecisionV0::NoCommand | GateDecisionV0::Denied { .. } => None,
                };
                if admitted.is_none_or(|intent| {
                    intent.clock_epoch != trace.subject_config.clock_epoch
                        || intent.source_epoch != trace.subject_config.subject_source_epoch
                        || intent.sequence != actuation.intent_sequence
                        || intent.permit_epoch != actuation.permit_epoch
                        || intent.command != actuation.command
                }) {
                    failures.push(format!(
                        "record {index} authorizes actuation without the matching admitted intent"
                    ));
                }
                if actuation.tick != record.tick || actuation.time_ns != record.time_ns {
                    failures.push(format!(
                        "record {index} actuation tick/time is inconsistent"
                    ));
                }
                let armed_permit = record
                    .subject_input
                    .safety_status
                    .as_ref()
                    .and_then(|frame| {
                        if !frame.payload.interlocks_clear {
                            return None;
                        }
                        match &frame.payload.controller_state {
                            SafetyControllerStateV0::Armed { permit_epoch } => Some(*permit_epoch),
                            SafetyControllerStateV0::Disarmed
                            | SafetyControllerStateV0::Tripped { .. } => None,
                        }
                    });
                if armed_permit != Some(actuation.permit_epoch)
                    || !matches!(
                        &record.safety_controller_after.controller_state,
                        SafetyControllerStateV0::Armed { permit_epoch }
                            if *permit_epoch == actuation.permit_epoch
                    )
                {
                    failures.push(format!(
                        "record {index} authorizes actuation outside matching Armed authority"
                    ));
                }
            }
            SafetyDispositionV0::Suppressed { .. } => {}
        }
    }

    validate_recorded_safety(trace, &mut failures);

    if let Some(last) = trace.records.last() {
        let recomputed = summarize(
            &trace.records,
            &last.safety_controller_after.controller_state,
            trace.summary.replayable,
        );
        if recomputed != trace.summary {
            failures.push("trace summary does not match its records".to_owned());
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

fn validate_recorded_safety(trace: &SemanticTraceV0, failures: &mut Vec<String>) {
    let mut safety =
        SafetyControllerSim::from_config(&trace.subject_config, trace.safety_policy.clone());
    for (index, record) in trace.records.iter().enumerate() {
        for action in &record.safety_actions {
            safety.apply_action(record.tick, record.time_ns, action);
        }
        for fault in &record.active_faults {
            match fault {
                FaultKindV0::EmergencyStop => safety.trip(SafetyTripReasonV0::EmergencyStop),
                FaultKindV0::DriveFault => safety.trip(SafetyTripReasonV0::DriveFault),
                FaultKindV0::RevokePermit => safety.disarm(),
                _ => {}
            }
        }
        let _ = safety.status(record.time_ns, trace.tick_period_ns);
        if matches!(
            record.subject_output.lifecycle,
            SubjectLifecycleV0::Faulted { .. }
        ) {
            safety.trip(SafetyTripReasonV0::SubjectFault);
        }
        let dropped = record
            .active_faults
            .iter()
            .any(|fault| matches!(fault, FaultKindV0::DropIntent));
        let intent = if dropped {
            None
        } else {
            match &record.subject_output.gate {
                GateDecisionV0::Admitted { intent, .. } => Some(intent),
                GateDecisionV0::NoCommand | GateDecisionV0::Denied { .. } => None,
            }
        };
        let expected = safety.evaluate(
            record.tick,
            record.time_ns,
            intent,
            &record.safety_observation,
        );
        if expected != record.safety_disposition {
            failures.push(format!(
                "record {index} safety disposition does not replay from recorded events"
            ));
        }
        if safety.snapshot() != record.safety_controller_after {
            failures.push(format!(
                "record {index} safety state does not replay from recorded events"
            ));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonReport {
    pub equal: bool,
    pub differences: Vec<String>,
}

#[must_use]
pub fn compare_semantic_traces(
    left: &SemanticTraceV0,
    right: &SemanticTraceV0,
) -> ComparisonReport {
    let mut differences = Vec::new();
    if let Err(failures) = validate_trace_evidence(left) {
        differences.extend(
            failures
                .into_iter()
                .map(|failure| format!("left trace: {failure}")),
        );
    }
    if let Err(failures) = validate_trace_evidence(right) {
        differences.extend(
            failures
                .into_iter()
                .map(|failure| format!("right trace: {failure}")),
        );
    }
    compare_trace_headers(left, right, &mut differences);

    if left.equality.require_bit_exact_mock_trace
        && left.plant_id == "so101-mock"
        && right.plant_id == "so101-mock"
    {
        if left.records != right.records {
            differences.push("bit-exact mock records differ".to_owned());
        }
        if left.summary != right.summary {
            differences.push("run summaries differ".to_owned());
        }
        return ComparisonReport {
            equal: differences.is_empty(),
            differences,
        };
    }

    if left.records.len() != right.records.len() {
        differences.push(format!(
            "record count differs: {} != {}",
            left.records.len(),
            right.records.len()
        ));
    }
    for (index, (a, b)) in left.records.iter().zip(&right.records).enumerate() {
        if a.tick != b.tick || a.time_ns != b.time_ns {
            differences.push(format!("tick/time differs at record {index}"));
        }
        if a.safety_actions != b.safety_actions || a.active_faults != b.active_faults {
            differences.push(format!("scheduled events differ at record {index}"));
        }
        compare_state(
            &a.safety_observation,
            &b.safety_observation,
            left.equality.physics_absolute_tolerance,
            &format!("safety observation at record {index}"),
            &mut differences,
        );
        compare_subject_input(
            &a.subject_input,
            &b.subject_input,
            &left.equality,
            index,
            &mut differences,
        );
        compare_subject_output(
            &a.subject_output,
            &b.subject_output,
            &left.equality,
            index,
            &mut differences,
        );
        compare_gate(
            &a.subject_output.gate,
            &b.subject_output.gate,
            left.equality.control_absolute_tolerance,
            index,
            &mut differences,
        );
        compare_disposition(
            &a.safety_disposition,
            &b.safety_disposition,
            left.equality.control_absolute_tolerance,
            index,
            &mut differences,
        );
        if a.safety_controller_after != b.safety_controller_after {
            differences.push(format!("safety-controller state differs at record {index}"));
        }
        compare_state(
            &a.plant_state_after,
            &b.plant_state_after,
            left.equality.physics_absolute_tolerance,
            &format!("plant state at record {index}"),
            &mut differences,
        );
    }
    if left.summary != right.summary {
        differences.push("run summaries differ".to_owned());
    }
    ComparisonReport {
        equal: differences.is_empty(),
        differences,
    }
}

fn compare_trace_headers(
    left: &SemanticTraceV0,
    right: &SemanticTraceV0,
    differences: &mut Vec<String>,
) {
    if left.schema_version != right.schema_version
        || left.contract != right.contract
        || left.profile != right.profile
        || left.scenario_name != right.scenario_name
        || left.scenario_sha256 != right.scenario_sha256
        || left.model_digest != right.model_digest
        || left.seed != right.seed
        || left.tick_period_ns != right.tick_period_ns
        || left.expected_ticks != right.expected_ticks
        || left.subject_config != right.subject_config
        || left.safety_policy != right.safety_policy
        || left.plant_id != right.plant_id
        || left.equality != right.equality
    {
        differences.push("trace provenance or execution profile differs".to_owned());
    }
}

/// Replays a subject and the independent safety controller from the exact
/// input/event stream captured in a semantic trace.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn replay_semantic_trace<S: Subject>(
    subject: &mut S,
    trace: &SemanticTraceV0,
) -> ComparisonReport {
    let mut differences = match validate_trace_evidence(trace) {
        Ok(()) => Vec::new(),
        Err(failures) => failures
            .into_iter()
            .map(|failure| format!("source trace: {failure}"))
            .collect(),
    };
    if !differences.is_empty() {
        return ComparisonReport {
            equal: false,
            differences,
        };
    }

    match catch_unwind(AssertUnwindSafe(|| subject.reset(&trace.subject_config))) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => differences.push(format!("replay subject reset failed: {error}")),
        Err(_) => differences.push("replay subject panicked during reset".to_owned()),
    }
    if !differences.is_empty() {
        return ComparisonReport {
            equal: false,
            differences,
        };
    }

    let mut safety =
        SafetyControllerSim::from_config(&trace.subject_config, trace.safety_policy.clone());
    for (index, record) in trace.records.iter().enumerate() {
        for action in &record.safety_actions {
            safety.apply_action(record.tick, record.time_ns, action);
        }
        for fault in &record.active_faults {
            match fault {
                FaultKindV0::EmergencyStop => safety.trip(SafetyTripReasonV0::EmergencyStop),
                FaultKindV0::DriveFault => safety.trip(SafetyTripReasonV0::DriveFault),
                FaultKindV0::RevokePermit => safety.disarm(),
                _ => {}
            }
        }
        let _ = safety.status(record.time_ns, trace.tick_period_ns);

        let replayed = match catch_unwind(AssertUnwindSafe(|| subject.step(&record.subject_input)))
        {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                differences.push(format!("replay subject failed at record {index}: {error}"));
                break;
            }
            Err(_) => {
                differences.push(format!("replay subject panicked at record {index}"));
                break;
            }
        };
        compare_subject_output(
            &record.subject_output,
            &replayed,
            &trace.equality,
            index,
            &mut differences,
        );
        compare_gate(
            &record.subject_output.gate,
            &replayed.gate,
            trace.equality.control_absolute_tolerance,
            index,
            &mut differences,
        );

        if matches!(replayed.lifecycle, SubjectLifecycleV0::Faulted { .. }) {
            safety.trip(SafetyTripReasonV0::SubjectFault);
        }
        let dropped = record
            .active_faults
            .iter()
            .any(|fault| matches!(fault, FaultKindV0::DropIntent));
        let intent = if dropped {
            None
        } else {
            match &replayed.gate {
                GateDecisionV0::Admitted { intent, .. } => Some(intent),
                GateDecisionV0::NoCommand | GateDecisionV0::Denied { .. } => None,
            }
        };
        let disposition = safety.evaluate(
            record.tick,
            record.time_ns,
            intent,
            &record.safety_observation,
        );
        compare_disposition(
            &record.safety_disposition,
            &disposition,
            trace.equality.control_absolute_tolerance,
            index,
            &mut differences,
        );
        if safety.snapshot() != record.safety_controller_after {
            differences.push(format!(
                "replayed safety-controller state differs at record {index}"
            ));
        }
    }

    ComparisonReport {
        equal: differences.is_empty(),
        differences,
    }
}

fn compare_subject_input(
    left: &SubjectInputV0,
    right: &SubjectInputV0,
    equality: &hefaos_testbench_contracts::EqualityProfileV0,
    index: usize,
    differences: &mut Vec<String>,
) {
    if left.tick != right.tick
        || left.time_ns != right.time_ns
        || left.safety_status != right.safety_status
        || left.proposal_fault != right.proposal_fault
    {
        differences.push(format!("subject input metadata differs at record {index}"));
    }
    match (&left.sensor, &right.sensor) {
        (Some(a), Some(b)) => {
            if a.clock_epoch != b.clock_epoch
                || a.source_epoch != b.source_epoch
                || a.sequence != b.sequence
                || a.captured_at_ns != b.captured_at_ns
                || a.valid_until_ns != b.valid_until_ns
                || a.validity != b.validity
            {
                differences.push(format!("sensor envelope differs at record {index}"));
            }
            compare_state(
                &a.payload,
                &b.payload,
                equality.physics_absolute_tolerance,
                &format!("sensor payload at record {index}"),
                differences,
            );
        }
        (None, None) => {}
        (Some(_), None) | (None, Some(_)) => {
            differences.push(format!("sensor presence differs at record {index}"));
        }
    }
    match (&left.setpoint, &right.setpoint) {
        (Some(a), Some(b)) => {
            if a.clock_epoch != b.clock_epoch
                || a.source_epoch != b.source_epoch
                || a.sequence != b.sequence
                || a.captured_at_ns != b.captured_at_ns
                || a.valid_until_ns != b.valid_until_ns
                || a.validity != b.validity
            {
                differences.push(format!("setpoint envelope differs at record {index}"));
            }
            compare_command(
                &a.payload,
                &b.payload,
                equality.control_absolute_tolerance,
                index,
                differences,
            );
        }
        (None, None) => {}
        (Some(_), None) | (None, Some(_)) => {
            differences.push(format!("setpoint presence differs at record {index}"));
        }
    }
}

fn compare_subject_output(
    left: &SubjectOutputV0,
    right: &SubjectOutputV0,
    equality: &hefaos_testbench_contracts::EqualityProfileV0,
    index: usize,
    differences: &mut Vec<String>,
) {
    if left.lifecycle != right.lifecycle {
        differences.push(format!("subject lifecycle differs at record {index}"));
    }
    match (&left.estimate, &right.estimate) {
        (Some(a), Some(b)) => compare_state(
            a,
            b,
            equality.physics_absolute_tolerance,
            &format!("subject estimate at record {index}"),
            differences,
        ),
        (None, None) => {}
        (Some(_), None) | (None, Some(_)) => {
            differences.push(format!(
                "subject estimate presence differs at record {index}"
            ));
        }
    }
    match (&left.control, &right.control) {
        (Some(a), Some(b)) => {
            if a.clock_epoch != b.clock_epoch
                || a.source_epoch != b.source_epoch
                || a.sequence != b.sequence
                || a.source_sensor_epoch != b.source_sensor_epoch
                || a.source_sensor_sequence != b.source_sensor_sequence
                || a.captured_at_ns != b.captured_at_ns
                || a.valid_until_ns != b.valid_until_ns
            {
                differences.push(format!(
                    "control proposal metadata differs at record {index}"
                ));
            }
            compare_command(
                &a.command,
                &b.command,
                equality.control_absolute_tolerance,
                index,
                differences,
            );
        }
        (None, None) => {}
        (Some(_), None) | (None, Some(_)) => {
            differences.push(format!(
                "control proposal presence differs at record {index}"
            ));
        }
    }
}

fn compare_gate(
    left: &GateDecisionV0,
    right: &GateDecisionV0,
    tolerance: f64,
    index: usize,
    differences: &mut Vec<String>,
) {
    match (left, right) {
        (GateDecisionV0::NoCommand, GateDecisionV0::NoCommand) => {}
        (GateDecisionV0::Denied { reason: a }, GateDecisionV0::Denied { reason: b }) if a == b => {}
        (
            GateDecisionV0::Admitted {
                intent: a,
                limited: limited_a,
            },
            GateDecisionV0::Admitted {
                intent: b,
                limited: limited_b,
            },
        ) => {
            if a.clock_epoch != b.clock_epoch
                || a.source_epoch != b.source_epoch
                || a.sequence != b.sequence
                || a.permit_epoch != b.permit_epoch
                || a.captured_at_ns != b.captured_at_ns
                || a.valid_until_ns != b.valid_until_ns
                || limited_a != limited_b
            {
                differences.push(format!("gate metadata differs at record {index}"));
            }
            compare_command(&a.command, &b.command, tolerance, index, differences);
        }
        _ => differences.push(format!("gate decision differs at record {index}")),
    }
}

fn compare_disposition(
    left: &SafetyDispositionV0,
    right: &SafetyDispositionV0,
    tolerance: f64,
    index: usize,
    differences: &mut Vec<String>,
) {
    match (left, right) {
        (
            SafetyDispositionV0::Suppressed { reason: a },
            SafetyDispositionV0::Suppressed { reason: b },
        ) if a == b => {}
        (
            SafetyDispositionV0::Authorized { actuation: a },
            SafetyDispositionV0::Authorized { actuation: b },
        ) => {
            if a.tick != b.tick
                || a.time_ns != b.time_ns
                || a.permit_epoch != b.permit_epoch
                || a.intent_sequence != b.intent_sequence
            {
                differences.push(format!("actuation metadata differs at record {index}"));
            }
            compare_command(&a.command, &b.command, tolerance, index, differences);
        }
        _ => differences.push(format!("safety disposition differs at record {index}")),
    }
}

fn compare_command(
    left: &So101CommandV0,
    right: &So101CommandV0,
    tolerance: f64,
    index: usize,
    differences: &mut Vec<String>,
) {
    if !left.is_finite()
        || !right.is_finite()
        || left
            .arm_position_rad
            .iter()
            .zip(right.arm_position_rad)
            .any(|(a, b)| (*a - b).abs() > tolerance)
        || (left.gripper_position - right.gripper_position).abs() > tolerance
    {
        differences.push(format!("command values differ at record {index}"));
    }
}

fn compare_state(
    left: &So101StateV0,
    right: &So101StateV0,
    tolerance: f64,
    label: &str,
    differences: &mut Vec<String>,
) {
    let differs = !left.is_finite()
        || !right.is_finite()
        || left
            .arm_position_rad
            .iter()
            .chain(&left.arm_velocity_rad_s)
            .zip(
                right
                    .arm_position_rad
                    .iter()
                    .chain(&right.arm_velocity_rad_s),
            )
            .any(|(a, b)| (*a - *b).abs() > tolerance)
        || (left.gripper_position - right.gripper_position).abs() > tolerance
        || (left.gripper_velocity_per_s - right.gripper_velocity_per_s).abs() > tolerance;
    if differs {
        differences.push(format!("{label} differs beyond tolerance {tolerance}"));
    }
}

/// Computes the canonical compact-JSON SHA-256 digest of typed trace evidence.
///
/// # Errors
///
/// Returns a serialization error if the typed evidence cannot be encoded.
pub fn semantic_trace_sha256(trace: &SemanticTraceV0) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(trace)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

#[must_use]
pub fn latency_summary(samples: &[u64]) -> LatencySummaryV0 {
    if samples.is_empty() {
        return LatencySummaryV0 {
            samples: 0,
            p50_ns: 0,
            p95_ns: 0,
            p99_ns: 0,
            p999_ns: 0,
            maximum_ns: 0,
        };
    }
    let mut ordered = samples.to_vec();
    ordered.sort_unstable();
    let percentile = |per_mille: usize| {
        let index = (ordered.len() - 1).saturating_mul(per_mille) / 1_000;
        ordered[index]
    };
    LatencySummaryV0 {
        samples: ordered.len() as u64,
        p50_ns: percentile(500),
        p95_ns: percentile(950),
        p99_ns: percentile(990),
        p999_ns: percentile(999),
        maximum_ns: ordered.last().copied().unwrap_or_default(),
    }
}

#[must_use]
pub fn benchmark_report(
    scenario: &ScenarioV0,
    scenario_sha256: &str,
    subject_id: &str,
    plant_id: &str,
    iterations: u64,
    semantic_failures: u64,
    samples: &[u64],
) -> BenchmarkReportV0 {
    BenchmarkReportV0 {
        schema_version: CONTRACT_SCHEMA_VERSION,
        scenario_name: scenario.name.clone(),
        scenario_sha256: scenario_sha256.to_owned(),
        subject_id: subject_id.to_owned(),
        plant_id: plant_id.to_owned(),
        iterations,
        semantic_failures,
        control_turn_latency: latency_summary(samples),
        wall_time_is_portable_gate: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hefaos_testbench_contracts::{JointLimitsV0, SafetyPolicyV0};

    fn limits() -> JointLimitsV0 {
        JointLimitsV0 {
            arm_position_rad: [[-1.0, 1.0]; 5],
            gripper_position: [0.0, 1.0],
        }
    }

    fn scenario() -> ScenarioV0 {
        ScenarioV0 {
            schema_version: CONTRACT_SCHEMA_VERSION,
            contract: CONTRACT_ID.to_owned(),
            profile: PROFILE_ID.to_owned(),
            name: "unit".to_owned(),
            model_digest: PINNED_SO101_MODEL_DIGEST.to_owned(),
            clock_epoch: ClockEpoch(1),
            sensor_source_epoch: SourceEpoch(1),
            setpoint_source_epoch: SourceEpoch(4),
            subject_source_epoch: SourceEpoch(2),
            safety_source_epoch: SourceEpoch(3),
            tick_period_ns: DurationNs(SO101_TICK_PERIOD_NS),
            ticks: Tick(3),
            seed: "0".to_owned(),
            initial_state: So101StateV0::zero(),
            setpoints: vec![hefaos_testbench_contracts::SetpointKeyframeV0 {
                at_tick: Tick(0),
                command: So101CommandV0::zero(),
            }],
            safety_actions: Vec::new(),
            faults: Vec::new(),
            gate_policy: hefaos_testbench_contracts::GatePolicyV0 {
                sensor_max_age_ns: DurationNs(10_000_000),
                safety_max_age_ns: DurationNs(10_000_000),
                setpoint_max_age_ns: DurationNs(10_000_000),
                proposal_ttl_ns: DurationNs(10_000_000),
                max_arm_step_rad: [0.1; 5],
                max_gripper_step: 0.1,
                limits: limits(),
            },
            safety_policy: SafetyPolicyV0 {
                intent_max_age_ns: DurationNs(10_000_000),
                heartbeat_timeout_ns: DurationNs(20_000_000),
                max_arm_step_rad: [0.1; 5],
                max_gripper_step: 0.1,
                limits: limits(),
            },
            mock_plant_policy: hefaos_testbench_contracts::MockPlantPolicyV0 {
                max_arm_velocity_rad_s: [1.0; 5],
                max_gripper_velocity_per_s: 1.0,
            },
            equality: hefaos_testbench_contracts::EqualityProfileV0 {
                control_absolute_tolerance: 0.0,
                physics_absolute_tolerance: 0.0,
                require_exact_discrete_trace: true,
                require_bit_exact_mock_trace: true,
            },
            expected: hefaos_testbench_contracts::ExpectedRunV0 {
                admitted_ticks: None,
                denied_by_reason: BTreeMap::new(),
                authorized_ticks: None,
                suppressed_ticks: Vec::new(),
                terminal_safety_state: None,
                final_state: None,
            },
        }
    }

    fn safety() -> SafetyControllerSim {
        SafetyControllerSim::new(&scenario())
    }

    #[test]
    fn virtual_clock_never_reads_wall_time() {
        let mut clock = VirtualClock::new(ClockEpoch(7));
        clock.advance(DurationNs(5_000_000));
        assert_eq!(clock.tick(), Tick(1));
        assert_eq!(clock.now(), VirtualTimeNs(5_000_000));
        assert_eq!(clock.epoch(), ClockEpoch(7));
    }

    #[test]
    fn arm_turn_never_authorizes_motion() {
        let mut controller = safety();
        controller.apply_action(Tick(0), VirtualTimeNs(0), &SafetyActionV0::Reset);
        controller.apply_action(Tick(1), VirtualTimeNs(5), &SafetyActionV0::Arm);
        let intent = ActuatorIntentV0 {
            clock_epoch: ClockEpoch(1),
            source_epoch: SourceEpoch(2),
            sequence: Sequence(0),
            permit_epoch: PermitEpoch(1),
            captured_at_ns: VirtualTimeNs(5),
            valid_until_ns: VirtualTimeNs(10),
            command: So101CommandV0::zero(),
        };
        assert_eq!(
            controller.evaluate(
                Tick(1),
                VirtualTimeNs(5),
                Some(&intent),
                &So101StateV0::zero(),
            ),
            SafetyDispositionV0::Suppressed {
                reason: SuppressionReasonV0::ArmTransition
            }
        );
        assert!(matches!(
            controller.evaluate(
                Tick(2),
                VirtualTimeNs(6),
                Some(&intent),
                &So101StateV0::zero(),
            ),
            SafetyDispositionV0::Authorized { .. }
        ));
    }

    #[test]
    fn trip_is_latched_until_clear_and_reset() {
        let mut controller = safety();
        controller.apply_action(Tick(0), VirtualTimeNs(0), &SafetyActionV0::Arm);
        controller.trip(SafetyTripReasonV0::EmergencyStop);
        controller.apply_action(Tick(1), VirtualTimeNs(5), &SafetyActionV0::ClearFaults);
        assert!(matches!(
            controller.state(),
            SafetyControllerStateV0::Tripped { .. }
        ));
        controller.apply_action(Tick(2), VirtualTimeNs(10), &SafetyActionV0::Reset);
        assert_eq!(controller.state(), &SafetyControllerStateV0::Disarmed);
    }

    fn valid_intent(sequence: u64) -> ActuatorIntentV0 {
        ActuatorIntentV0 {
            clock_epoch: ClockEpoch(1),
            source_epoch: SourceEpoch(2),
            sequence: Sequence(sequence),
            permit_epoch: PermitEpoch(1),
            captured_at_ns: VirtualTimeNs(5_000_000),
            valid_until_ns: VirtualTimeNs(15_000_000),
            command: So101CommandV0::zero(),
        }
    }

    fn armed_controller() -> SafetyControllerSim {
        let mut controller = safety();
        controller.apply_action(Tick(0), VirtualTimeNs(0), &SafetyActionV0::Arm);
        controller
    }

    #[derive(Debug, Clone, Copy)]
    enum ScriptedBehavior {
        Error,
        Panic,
        MotionlessIntent,
        UnsafeIntent,
    }

    #[derive(Debug)]
    struct ScriptedSubject {
        behavior: ScriptedBehavior,
        config: Option<SubjectConfigV0>,
    }

    impl ScriptedSubject {
        const fn new(behavior: ScriptedBehavior) -> Self {
            Self {
                behavior,
                config: None,
            }
        }
    }

    impl Subject for ScriptedSubject {
        fn id(&self) -> &'static str {
            "scripted-adversary/v0"
        }

        fn reset(&mut self, config: &SubjectConfigV0) -> Result<(), SubjectError> {
            self.config = Some(config.clone());
            Ok(())
        }

        fn step(&mut self, input: &SubjectInputV0) -> Result<SubjectOutputV0, SubjectError> {
            match self.behavior {
                ScriptedBehavior::Error => Err(SubjectError::Step("injected error".to_owned())),
                ScriptedBehavior::Panic => panic!("injected panic"),
                ScriptedBehavior::MotionlessIntent => {
                    let config = self.config.as_ref().expect("subject was reset");
                    let gate = match input
                        .safety_status
                        .as_ref()
                        .map(|frame| &frame.payload.controller_state)
                    {
                        Some(SafetyControllerStateV0::Armed { permit_epoch }) => {
                            GateDecisionV0::Admitted {
                                intent: ActuatorIntentV0 {
                                    clock_epoch: config.clock_epoch,
                                    source_epoch: config.subject_source_epoch,
                                    sequence: Sequence(input.tick.0),
                                    permit_epoch: *permit_epoch,
                                    captured_at_ns: input.time_ns,
                                    valid_until_ns: VirtualTimeNs(
                                        input.time_ns.0.saturating_add(10_000_000),
                                    ),
                                    command: So101CommandV0::zero(),
                                },
                                limited: false,
                            }
                        }
                        Some(
                            SafetyControllerStateV0::Disarmed
                            | SafetyControllerStateV0::Tripped { .. },
                        )
                        | None => GateDecisionV0::Denied {
                            reason: GateDeniedReasonV0::SafetyNotArmed,
                        },
                    };
                    Ok(SubjectOutputV0 {
                        lifecycle: SubjectLifecycleV0::Running,
                        estimate: input.sensor.as_ref().map(|frame| frame.payload.clone()),
                        control: None,
                        gate,
                    })
                }
                ScriptedBehavior::UnsafeIntent => {
                    let config = self.config.as_ref().expect("subject was reset");
                    let mut command = So101CommandV0::zero();
                    command.arm_position_rad[0] = 2.0;
                    Ok(SubjectOutputV0 {
                        lifecycle: SubjectLifecycleV0::Running,
                        estimate: input.sensor.as_ref().map(|frame| frame.payload.clone()),
                        control: None,
                        gate: GateDecisionV0::Admitted {
                            intent: ActuatorIntentV0 {
                                clock_epoch: config.clock_epoch,
                                source_epoch: config.subject_source_epoch,
                                sequence: Sequence(input.tick.0),
                                permit_epoch: PermitEpoch(1),
                                captured_at_ns: input.time_ns,
                                valid_until_ns: VirtualTimeNs(
                                    input.time_ns.0.saturating_add(10_000_000),
                                ),
                                command,
                            },
                            limited: false,
                        },
                    })
                }
            }
        }
    }

    #[derive(Debug)]
    struct StaticPlant {
        state: So101StateV0,
    }

    impl Default for StaticPlant {
        fn default() -> Self {
            Self {
                state: So101StateV0::zero(),
            }
        }
    }

    impl Plant for StaticPlant {
        fn id(&self) -> &'static str {
            "static-test-plant"
        }

        fn model_digest(&self) -> &'static str {
            PINNED_SO101_MODEL_DIGEST
        }

        fn reset(&mut self, initial: &So101StateV0) -> Result<(), PlantError> {
            self.state = initial.clone();
            Ok(())
        }

        fn observe(
            &self,
            _tick: Tick,
            now: VirtualTimeNs,
            clock_epoch: ClockEpoch,
            source_epoch: SourceEpoch,
            sequence: Sequence,
        ) -> Result<SensorFrameV0, PlantError> {
            Ok(EnvelopeV0 {
                clock_epoch,
                source_epoch,
                sequence,
                captured_at_ns: now,
                valid_until_ns: VirtualTimeNs(now.0.saturating_add(SO101_TICK_PERIOD_NS)),
                validity: ValidityV0::Valid,
                payload: self.state.clone(),
            })
        }

        fn apply(&mut self, actuation: &AppliedActuationV0) -> Result<(), PlantError> {
            self.state.arm_position_rad = actuation.command.arm_position_rad;
            self.state.gripper_position = actuation.command.gripper_position;
            Ok(())
        }

        fn advance(&mut self, _duration: DurationNs) -> Result<(), PlantError> {
            Ok(())
        }

        fn state(&self) -> Result<So101StateV0, PlantError> {
            Ok(self.state.clone())
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn safety_rejects_adversarial_intent_matrix() {
        let cases = [
            (
                {
                    let mut intent = valid_intent(0);
                    intent.clock_epoch = ClockEpoch(99);
                    intent
                },
                SuppressionReasonV0::WrongClockEpoch,
            ),
            (
                {
                    let mut intent = valid_intent(0);
                    intent.source_epoch = SourceEpoch(99);
                    intent
                },
                SuppressionReasonV0::WrongSourceEpoch,
            ),
            (
                {
                    let mut intent = valid_intent(0);
                    intent.permit_epoch = PermitEpoch(99);
                    intent
                },
                SuppressionReasonV0::WrongPermitEpoch,
            ),
            (
                {
                    let mut intent = valid_intent(0);
                    intent.captured_at_ns = VirtualTimeNs(5_000_001);
                    intent
                },
                SuppressionReasonV0::ExpiredIntent,
            ),
            (
                {
                    let mut intent = valid_intent(0);
                    intent.valid_until_ns = VirtualTimeNs(5_000_000);
                    intent
                },
                SuppressionReasonV0::ExpiredIntent,
            ),
        ];
        for (intent, expected) in cases {
            let mut controller = armed_controller();
            assert_eq!(
                controller.evaluate(
                    Tick(1),
                    VirtualTimeNs(5_000_000),
                    Some(&intent),
                    &So101StateV0::zero(),
                ),
                SafetyDispositionV0::Suppressed { reason: expected }
            );
        }

        let mut age_boundary = valid_intent(0);
        age_boundary.captured_at_ns = VirtualTimeNs(0);
        age_boundary.valid_until_ns = VirtualTimeNs(20_000_000);
        assert_eq!(
            armed_controller().evaluate(
                Tick(2),
                VirtualTimeNs(10_000_000),
                Some(&age_boundary),
                &So101StateV0::zero(),
            ),
            SafetyDispositionV0::Suppressed {
                reason: SuppressionReasonV0::ExpiredIntent
            }
        );

        for expected in [
            SuppressionReasonV0::NonFinite,
            SuppressionReasonV0::OutOfRange,
            SuppressionReasonV0::SlewLimitExceeded,
        ] {
            let mut intent = valid_intent(0);
            match expected {
                SuppressionReasonV0::NonFinite => {
                    intent.command.arm_position_rad[0] = f64::NAN;
                }
                SuppressionReasonV0::OutOfRange => {
                    intent.command.arm_position_rad[0] = 2.0;
                }
                SuppressionReasonV0::SlewLimitExceeded => {
                    intent.command.arm_position_rad[0] = 0.2;
                }
                _ => unreachable!("test matrix only contains command validation failures"),
            }
            let mut controller = armed_controller();
            assert_eq!(
                controller.evaluate(
                    Tick(1),
                    VirtualTimeNs(5_000_000),
                    Some(&intent),
                    &So101StateV0::zero(),
                ),
                SafetyDispositionV0::Suppressed { reason: expected }
            );
            assert!(matches!(
                controller.state(),
                SafetyControllerStateV0::Tripped {
                    reason: SafetyTripReasonV0::CommandInvalid
                }
            ));
        }

        let mut controller = armed_controller();
        let first = valid_intent(5);
        assert!(matches!(
            controller.evaluate(
                Tick(1),
                VirtualTimeNs(5_000_000),
                Some(&first),
                &So101StateV0::zero(),
            ),
            SafetyDispositionV0::Authorized { .. }
        ));
        assert_eq!(
            controller.evaluate(
                Tick(2),
                VirtualTimeNs(6_000_000),
                Some(&first),
                &So101StateV0::zero(),
            ),
            SafetyDispositionV0::Suppressed {
                reason: SuppressionReasonV0::DuplicateIntent
            }
        );
        let earlier = valid_intent(4);
        assert_eq!(
            controller.evaluate(
                Tick(3),
                VirtualTimeNs(7_000_000),
                Some(&earlier),
                &So101StateV0::zero(),
            ),
            SafetyDispositionV0::Suppressed {
                reason: SuppressionReasonV0::OutOfOrderIntent
            }
        );
    }

    #[test]
    fn watchdog_and_trip_latch_are_half_open_and_cannot_be_disarmed_away() {
        let mut controller = armed_controller();
        assert_eq!(
            controller.evaluate(
                Tick(1),
                VirtualTimeNs(5_000_000),
                None,
                &So101StateV0::zero(),
            ),
            SafetyDispositionV0::Suppressed {
                reason: SuppressionReasonV0::MissingIntent
            }
        );
        assert_eq!(
            controller.evaluate(
                Tick(4),
                VirtualTimeNs(20_000_000),
                None,
                &So101StateV0::zero(),
            ),
            SafetyDispositionV0::Suppressed {
                reason: SuppressionReasonV0::Tripped
            }
        );
        assert!(matches!(
            controller.state(),
            SafetyControllerStateV0::Tripped {
                reason: SafetyTripReasonV0::HeartbeatExpired
            }
        ));
        controller.apply_action(Tick(5), VirtualTimeNs(25_000_000), &SafetyActionV0::Disarm);
        assert!(matches!(
            controller.state(),
            SafetyControllerStateV0::Tripped { .. }
        ));
    }

    #[test]
    fn slew_guard_uses_the_authoritative_observation() {
        let mut controller = armed_controller();
        let mut observation = So101StateV0::zero();
        observation.arm_position_rad[0] = 0.000_000_2;
        let mut intent = valid_intent(0);
        intent.command.arm_position_rad[0] = observation.arm_position_rad[0] + 0.1;

        assert!(matches!(
            controller.evaluate(
                Tick(1),
                VirtualTimeNs(5_000_000),
                Some(&intent),
                &observation,
            ),
            SafetyDispositionV0::Authorized { .. }
        ));
    }

    #[test]
    fn safety_trips_on_out_of_range_authoritative_feedback() {
        let mut controller = armed_controller();
        let mut observation = So101StateV0::zero();
        observation.arm_position_rad[0] = 1.01;
        let mut intent = valid_intent(0);
        intent.command.arm_position_rad[0] = 1.0;

        assert_eq!(
            controller.evaluate(
                Tick(1),
                VirtualTimeNs(5_000_000),
                Some(&intent),
                &observation,
            ),
            SafetyDispositionV0::Suppressed {
                reason: SuppressionReasonV0::FeedbackInvalid
            }
        );
        assert_eq!(
            controller.state(),
            &SafetyControllerStateV0::Tripped {
                reason: SafetyTripReasonV0::FeedbackInvalid
            }
        );
    }

    #[test]
    fn scenario_rejects_a_safety_policy_looser_than_the_subject_gate() {
        let mut loose_limits = scenario();
        loose_limits.safety_policy.limits.arm_position_rad[0][1] = 1.1;
        assert!(validate_scenario(&loose_limits).is_err());

        let mut loose_slew = scenario();
        loose_slew.safety_policy.max_arm_step_rad[0] = 0.2;
        assert!(validate_scenario(&loose_slew).is_err());

        let mut loose_age = scenario();
        loose_age.safety_policy.intent_max_age_ns = DurationNs(10_000_001);
        assert!(validate_scenario(&loose_age).is_err());
    }

    #[test]
    fn nominal_acceptance_rejects_a_safe_but_motionless_subject() {
        let mut scenario = scenario();
        scenario.setpoints[0].command.arm_position_rad[0] = 0.1;
        scenario.safety_actions = vec![
            hefaos_testbench_contracts::ScheduledSafetyActionV0 {
                at_tick: Tick(0),
                action: SafetyActionV0::Reset,
            },
            hefaos_testbench_contracts::ScheduledSafetyActionV0 {
                at_tick: Tick(1),
                action: SafetyActionV0::Arm,
            },
        ];
        scenario.expected.admitted_ticks = Some(2);
        scenario.expected.denied_by_reason =
            BTreeMap::from([(GateDeniedReasonV0::SafetyNotArmed, 1)]);
        scenario.expected.authorized_ticks = Some(1);
        scenario.expected.suppressed_ticks = vec![Tick(0), Tick(1)];
        scenario.expected.terminal_safety_state = Some(ExpectedSafetyStateV0::Armed);
        let mut expected_state = So101StateV0::zero();
        expected_state.arm_position_rad[0] = 0.1;
        scenario.expected.final_state = Some(hefaos_testbench_contracts::ExpectedStateV0 {
            state: expected_state,
            absolute_tolerance: 0.0,
        });

        let outcome = Runner::new(
            ScriptedSubject::new(ScriptedBehavior::MotionlessIntent),
            StaticPlant::default(),
        )
        .run(&scenario, &"0".repeat(64))
        .expect("run motionless subject");

        assert!(!outcome.verdict.passed);
        assert_eq!(
            outcome.verdict.failures,
            vec!["final state differs beyond tolerance 0"]
        );
    }

    #[test]
    fn task_failure_evidence_requires_an_immediate_latched_subject_fault() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../goldens/v0/subject_task_failure.semantic-trace.json");
        let bytes = std::fs::read(path).expect("read task-failure golden");
        let mut trace: SemanticTraceV0 =
            serde_json::from_slice(&bytes).expect("decode task-failure golden");
        validate_trace_evidence(&trace).expect("golden task-failure evidence is valid");

        for record in trace.records.iter_mut().skip(3) {
            record.subject_output.lifecycle = SubjectLifecycleV0::Running;
        }
        let failures =
            validate_trace_evidence(&trace).expect_err("running lifecycle must fail closed");
        assert!(failures.iter().any(|failure| {
            failure.contains("does not preserve the injected task-fault lifecycle")
        }));
    }

    #[test]
    fn runner_never_applies_an_adversarial_out_of_range_intent() {
        let mut scenario = scenario();
        scenario.safety_actions = vec![
            hefaos_testbench_contracts::ScheduledSafetyActionV0 {
                at_tick: Tick(0),
                action: SafetyActionV0::Reset,
            },
            hefaos_testbench_contracts::ScheduledSafetyActionV0 {
                at_tick: Tick(1),
                action: SafetyActionV0::Arm,
            },
        ];
        scenario.expected.admitted_ticks = Some(3);
        scenario.expected.authorized_ticks = Some(0);
        scenario.expected.suppressed_ticks = vec![Tick(0), Tick(1), Tick(2)];
        scenario.expected.terminal_safety_state = Some(ExpectedSafetyStateV0::Tripped);

        let outcome = Runner::new(
            ScriptedSubject::new(ScriptedBehavior::UnsafeIntent),
            StaticPlant::default(),
        )
        .run(&scenario, &"0".repeat(64))
        .expect("run adversarial subject");
        assert!(outcome.verdict.passed, "{:?}", outcome.verdict.failures);
        assert_eq!(outcome.trace.summary.authorized_ticks, 0);
        assert_eq!(
            outcome.trace.records.last().unwrap().plant_state_after,
            So101StateV0::zero()
        );
        assert!(matches!(
            outcome
                .trace
                .records
                .last()
                .unwrap()
                .safety_controller_after
                .controller_state,
            SafetyControllerStateV0::Tripped {
                reason: SafetyTripReasonV0::CommandInvalid
            }
        ));

        validate_trace_evidence(&outcome.trace).expect("runner emits valid evidence");
        let mut forged = outcome.trace.clone();
        forged.records[1].safety_observation.arm_position_rad[0] = 0.01;
        let failures = validate_trace_evidence(&forged).expect_err("forged baseline must fail");
        assert!(failures.iter().any(|failure| {
            failure.contains("safety observation is not the preceding authoritative plant state")
        }));
    }

    #[test]
    fn subject_errors_and_panics_fail_closed_and_are_never_replayable() {
        for behavior in [ScriptedBehavior::Error, ScriptedBehavior::Panic] {
            let mut scenario = scenario();
            scenario.safety_actions = vec![
                hefaos_testbench_contracts::ScheduledSafetyActionV0 {
                    at_tick: Tick(0),
                    action: SafetyActionV0::Reset,
                },
                hefaos_testbench_contracts::ScheduledSafetyActionV0 {
                    at_tick: Tick(1),
                    action: SafetyActionV0::Arm,
                },
            ];
            scenario.expected.denied_by_reason =
                BTreeMap::from([(GateDeniedReasonV0::TaskError, 3)]);
            scenario.expected.authorized_ticks = Some(0);
            scenario.expected.suppressed_ticks = vec![Tick(0), Tick(1), Tick(2)];
            scenario.expected.terminal_safety_state = Some(ExpectedSafetyStateV0::Tripped);

            let outcome = Runner::new(ScriptedSubject::new(behavior), StaticPlant::default())
                .run(&scenario, &"0".repeat(64))
                .expect("runner converts subject failure to evidence");
            assert!(!outcome.trace.summary.replayable);
            assert!(!outcome.verdict.passed);
            assert_eq!(outcome.trace.summary.authorized_ticks, 0);
            assert!(matches!(
                outcome
                    .trace
                    .records
                    .first()
                    .unwrap()
                    .safety_controller_after
                    .controller_state,
                SafetyControllerStateV0::Tripped {
                    reason: SafetyTripReasonV0::SubjectFault
                }
            ));
        }
    }

    #[test]
    fn latency_percentiles_are_deterministic() {
        let summary = latency_summary(&[50, 10, 30, 40, 20]);
        assert_eq!(summary.p50_ns, 30);
        assert_eq!(summary.maximum_ns, 50);
    }
}
