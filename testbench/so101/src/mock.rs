use crate::{
    ARM_JOINT_COUNT, CONTROL_RANGES_RAD, So101PlantError, TIMESTEP_NANOSECONDS,
    model::values_are_finite,
};
use hefaos_testbench_contracts::{
    AppliedActuationV0, ClockEpoch, DurationNs, EnvelopeV0, MockPlantPolicyV0, ScenarioV0,
    SensorFrameV0, Sequence, So101StateV0, SourceEpoch, ValidityV0, VirtualTimeNs,
};

/// Deterministic dependency-free SO-101 plant used by the default testbench.
///
/// The plant is intentionally kinematic: on each 5 ms advance it moves to the
/// last authorized position target and derives velocity from the exact delta.
/// That gives fault/replay tests an unambiguous oracle without pretending to be
/// a second physics engine.
#[derive(Debug, Clone)]
pub struct MockSo101Plant {
    id: &'static str,
    arm_position_rad: [f64; ARM_JOINT_COUNT],
    arm_velocity_rad_s: [f64; ARM_JOINT_COUNT],
    gripper_position: f64,
    gripper_velocity_per_s: f64,
    pending_target: Option<([f64; ARM_JOINT_COUNT], f64)>,
    policy: MockPlantPolicyV0,
}

impl MockSo101Plant {
    /// Constructs a plant with deterministic per-channel velocity limits and
    /// validates the scenario period at the backend boundary.
    ///
    /// # Errors
    ///
    /// Returns [`So101PlantError`] if a velocity limit is invalid or the period
    /// is not exactly 5 ms.
    pub fn new(
        policy: MockPlantPolicyV0,
        tick_period: DurationNs,
    ) -> Result<Self, So101PlantError> {
        validate_policy(&policy)?;
        if tick_period.get() != TIMESTEP_NANOSECONDS {
            return Err(So101PlantError::InvalidTimestep {
                actual_ns: tick_period.get(),
            });
        }
        Ok(Self {
            id: "so101-mock",
            arm_position_rad: [0.0; ARM_JOINT_COUNT],
            arm_velocity_rad_s: [0.0; ARM_JOINT_COUNT],
            gripper_position: 0.0,
            gripper_velocity_per_s: 0.0,
            pending_target: None,
            policy,
        })
    }

    /// Constructs the mock directly from the scenario fields it owns.
    ///
    /// # Errors
    ///
    /// Returns [`So101PlantError`] if the scenario's mock policy or period is invalid.
    pub fn from_scenario(scenario: &ScenarioV0) -> Result<Self, So101PlantError> {
        Self::new(scenario.mock_plant_policy.clone(), scenario.tick_period_ns)
    }

    pub(crate) const fn plant_id(&self) -> &'static str {
        self.id
    }

    /// Resets all observable and target state from the versioned contract.
    ///
    /// # Errors
    ///
    /// Returns [`So101PlantError`] for non-finite or out-of-range state.
    pub fn reset_state(&mut self, initial: &So101StateV0) -> Result<(), So101PlantError> {
        self.reset_values(
            initial.arm_position_rad,
            initial.arm_velocity_rad_s,
            initial.gripper_position,
            initial.gripper_velocity_per_s,
        )
    }

    /// Stores the last safety-authorized actuation target.
    ///
    /// # Errors
    ///
    /// Returns [`So101PlantError`] for non-finite or out-of-range targets.
    pub fn apply_actuation(
        &mut self,
        actuation: &AppliedActuationV0,
    ) -> Result<(), So101PlantError> {
        self.apply_values(
            actuation.command.arm_position_rad,
            actuation.command.gripper_position,
        )
    }

    /// Advances the deterministic plant by exactly one model period.
    ///
    /// # Errors
    ///
    /// Returns [`So101PlantError`] unless `dt` is exactly 5 ms.
    pub fn advance_duration(&mut self, dt: DurationNs) -> Result<(), So101PlantError> {
        self.advance_ns(dt.get())
    }

    /// Returns the current versioned state without timestamp metadata.
    #[must_use]
    pub fn state(&self) -> So101StateV0 {
        let (arm_position_rad, arm_velocity_rad_s, gripper_position, gripper_velocity_per_s) =
            self.state_values();
        So101StateV0 {
            arm_position_rad,
            arm_velocity_rad_s,
            gripper_position,
            gripper_velocity_per_s,
        }
    }

    /// Wraps current state in a valid one-period sensor envelope.
    #[must_use]
    pub fn sensor_frame(
        &self,
        now: VirtualTimeNs,
        clock_epoch: ClockEpoch,
        source_epoch: SourceEpoch,
        sequence: Sequence,
    ) -> SensorFrameV0 {
        EnvelopeV0 {
            clock_epoch,
            source_epoch,
            sequence,
            captured_at_ns: now,
            valid_until_ns: VirtualTimeNs(now.get().saturating_add(TIMESTEP_NANOSECONDS)),
            validity: ValidityV0::Valid,
            payload: self.state(),
        }
    }

    pub(crate) fn reset_values(
        &mut self,
        arm_position_rad: [f64; ARM_JOINT_COUNT],
        arm_velocity_rad_s: [f64; ARM_JOINT_COUNT],
        gripper_position: f64,
        gripper_velocity_per_s: f64,
    ) -> Result<(), So101PlantError> {
        validate_state(
            &arm_position_rad,
            &arm_velocity_rad_s,
            gripper_position,
            gripper_velocity_per_s,
        )?;
        self.arm_position_rad = arm_position_rad;
        self.arm_velocity_rad_s = arm_velocity_rad_s;
        self.gripper_position = gripper_position;
        self.gripper_velocity_per_s = gripper_velocity_per_s;
        self.pending_target = None;
        Ok(())
    }

    pub(crate) fn apply_values(
        &mut self,
        arm_position_rad: [f64; ARM_JOINT_COUNT],
        gripper_position: f64,
    ) -> Result<(), So101PlantError> {
        validate_targets(&arm_position_rad, gripper_position)?;
        self.pending_target = Some((arm_position_rad, gripper_position));
        Ok(())
    }

    pub(crate) fn advance_ns(&mut self, dt_ns: u64) -> Result<(), So101PlantError> {
        if dt_ns != TIMESTEP_NANOSECONDS {
            return Err(So101PlantError::InvalidTimestep { actual_ns: dt_ns });
        }

        let dt_seconds = crate::TIMESTEP_SECONDS;
        let (target_arm_position_rad, target_gripper_position) = self
            .pending_target
            .take()
            .unwrap_or((self.arm_position_rad, self.gripper_position));
        for (index, target) in target_arm_position_rad.iter().copied().enumerate() {
            let delta = target - self.arm_position_rad[index];
            let max_delta = self.policy.max_arm_velocity_rad_s[index] * dt_seconds;
            let applied_delta = delta.clamp(-max_delta, max_delta);
            self.arm_position_rad[index] += applied_delta;
            self.arm_velocity_rad_s[index] = applied_delta / dt_seconds;
        }
        let gripper_delta = target_gripper_position - self.gripper_position;
        let max_gripper_delta = self.policy.max_gripper_velocity_per_s * dt_seconds;
        let applied_gripper_delta = gripper_delta.clamp(-max_gripper_delta, max_gripper_delta);
        self.gripper_position += applied_gripper_delta;
        self.gripper_velocity_per_s = applied_gripper_delta / dt_seconds;
        Ok(())
    }

    pub(crate) fn state_values(
        &self,
    ) -> ([f64; ARM_JOINT_COUNT], [f64; ARM_JOINT_COUNT], f64, f64) {
        (
            self.arm_position_rad,
            self.arm_velocity_rad_s,
            self.gripper_position,
            self.gripper_velocity_per_s,
        )
    }
}

impl Default for MockSo101Plant {
    fn default() -> Self {
        Self {
            id: "so101-mock",
            arm_position_rad: [0.0; ARM_JOINT_COUNT],
            arm_velocity_rad_s: [0.0; ARM_JOINT_COUNT],
            gripper_position: 0.0,
            gripper_velocity_per_s: 0.0,
            pending_target: None,
            policy: MockPlantPolicyV0 {
                max_arm_velocity_rad_s: [1.0; ARM_JOINT_COUNT],
                max_gripper_velocity_per_s: 1.0,
            },
        }
    }
}

pub(crate) fn validate_targets(
    arm_position_rad: &[f64; ARM_JOINT_COUNT],
    gripper_position: f64,
) -> Result<(), So101PlantError> {
    if !values_are_finite(arm_position_rad) {
        return Err(So101PlantError::NonFinite {
            channel: "arm_position_rad",
        });
    }
    if !gripper_position.is_finite() {
        return Err(So101PlantError::NonFinite {
            channel: "gripper_position",
        });
    }

    for (index, (value, [minimum, maximum])) in arm_position_rad
        .iter()
        .zip(CONTROL_RANGES_RAD[..ARM_JOINT_COUNT].iter().copied())
        .enumerate()
    {
        if !(minimum..=maximum).contains(value) {
            return Err(So101PlantError::OutOfRange {
                channel: "arm_position_rad",
                index,
                value: *value,
                minimum,
                maximum,
            });
        }
    }

    if !(0.0..=1.0).contains(&gripper_position) {
        return Err(So101PlantError::OutOfRange {
            channel: "gripper_position",
            index: 0,
            value: gripper_position,
            minimum: 0.0,
            maximum: 1.0,
        });
    }
    Ok(())
}

pub(crate) fn validate_state(
    arm_position_rad: &[f64; ARM_JOINT_COUNT],
    arm_velocity_rad_s: &[f64; ARM_JOINT_COUNT],
    gripper_position: f64,
    gripper_velocity_per_s: f64,
) -> Result<(), So101PlantError> {
    validate_targets(arm_position_rad, gripper_position)?;
    if !values_are_finite(arm_velocity_rad_s) {
        return Err(So101PlantError::NonFinite {
            channel: "arm_velocity_rad_s",
        });
    }
    if !gripper_velocity_per_s.is_finite() {
        return Err(So101PlantError::NonFinite {
            channel: "gripper_velocity_per_s",
        });
    }
    Ok(())
}

fn validate_policy(policy: &MockPlantPolicyV0) -> Result<(), So101PlantError> {
    if !values_are_finite(&policy.max_arm_velocity_rad_s) {
        return Err(So101PlantError::NonFinite {
            channel: "max_arm_velocity_rad_s",
        });
    }
    if !policy.max_gripper_velocity_per_s.is_finite() {
        return Err(So101PlantError::NonFinite {
            channel: "max_gripper_velocity_per_s",
        });
    }
    for (index, value) in policy.max_arm_velocity_rad_s.iter().enumerate() {
        if *value < 0.0 {
            return Err(So101PlantError::OutOfRange {
                channel: "max_arm_velocity_rad_s",
                index,
                value: *value,
                minimum: 0.0,
                maximum: f64::INFINITY,
            });
        }
    }
    if policy.max_gripper_velocity_per_s < 0.0 {
        return Err(So101PlantError::OutOfRange {
            channel: "max_gripper_velocity_per_s",
            index: 0,
            value: policy.max_gripper_velocity_per_s,
            minimum: 0.0,
            maximum: f64::INFINITY,
        });
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // The mock promises bit-exact deterministic replay.
mod tests {
    use super::*;
    use hefaos_testbench_contracts::{PermitEpoch, So101CommandV0, Tick};

    #[test]
    fn mock_is_exact_and_deterministic() {
        let mut first = MockSo101Plant::default();
        let mut second = first.clone();
        let arm = [0.1, -0.2, 0.3, -0.4, 0.5];

        first.apply_values(arm, 0.75).unwrap();
        second.apply_values(arm, 0.75).unwrap();
        first.advance_ns(TIMESTEP_NANOSECONDS).unwrap();
        second.advance_ns(TIMESTEP_NANOSECONDS).unwrap();

        assert_eq!(first.state_values(), second.state_values());
        assert_eq!(
            first.state_values().0,
            [0.005, -0.005, 0.005, -0.005, 0.005]
        );
        assert_eq!(first.state_values().2, 0.005);
        assert_eq!(first.state_values().1, [1.0, -1.0, 1.0, -1.0, 1.0]);
        assert_eq!(first.state_values().3, 1.0);
    }

    #[test]
    fn mock_rejects_invalid_inputs_and_periods() {
        let mut plant = MockSo101Plant::default();
        let err = plant
            .apply_values([f64::NAN, 0.0, 0.0, 0.0, 0.0], 0.0)
            .unwrap_err();
        assert!(matches!(err, So101PlantError::NonFinite { .. }));

        let err = plant.advance_ns(1_000_000).unwrap_err();
        assert!(matches!(err, So101PlantError::InvalidTimestep { .. }));
    }

    #[test]
    fn advance_consumes_pending_command_and_holds_without_one() {
        let mut plant = MockSo101Plant::default();
        plant.apply_values([0.1; ARM_JOINT_COUNT], 0.5).unwrap();
        plant.advance_ns(TIMESTEP_NANOSECONDS).unwrap();
        let moved = plant.state_values();

        plant.advance_ns(TIMESTEP_NANOSECONDS).unwrap();
        let held = plant.state_values();
        assert_eq!(held.0, moved.0);
        assert_eq!(held.2, moved.2);
        assert_eq!(held.1, [0.0; ARM_JOINT_COUNT]);
        assert_eq!(held.3, 0.0);
    }

    #[test]
    fn contract_methods_preserve_metadata_and_apply_only_authorized_values() {
        let mut plant = MockSo101Plant::default();
        plant.reset_state(&So101StateV0::zero()).unwrap();
        let actuation = AppliedActuationV0 {
            tick: Tick(4),
            time_ns: VirtualTimeNs(20_000_000),
            permit_epoch: PermitEpoch(7),
            intent_sequence: Sequence(11),
            command: So101CommandV0 {
                arm_position_rad: [0.2, -0.2, 0.2, -0.2, 0.2],
                gripper_position: 0.5,
            },
        };
        plant.apply_actuation(&actuation).unwrap();
        plant
            .advance_duration(DurationNs(TIMESTEP_NANOSECONDS))
            .unwrap();

        let frame = plant.sensor_frame(
            VirtualTimeNs(25_000_000),
            ClockEpoch(3),
            SourceEpoch(5),
            Sequence(12),
        );
        assert_eq!(frame.clock_epoch, ClockEpoch(3));
        assert_eq!(frame.source_epoch, SourceEpoch(5));
        assert_eq!(frame.sequence, Sequence(12));
        assert_eq!(frame.captured_at_ns, VirtualTimeNs(25_000_000));
        assert_eq!(frame.valid_until_ns, VirtualTimeNs(30_000_000));
        assert_eq!(frame.payload, plant.state());
    }
}
