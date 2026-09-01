use std::{env, fs, path::Path};

use hefaos_testbench_contracts::{
    AppliedActuationV0, ClockEpoch, DurationNs, EnvelopeV0, SensorFrameV0, Sequence, So101StateV0,
    SourceEpoch, ValidityV0, VirtualTimeNs,
};
use mujoco_rs::prelude::{MjData, MjModel, MjtObj};
use sha2::{Digest, Sha256};

use crate::{
    ACTUATOR_COUNT, ARM_JOINT_COUNT, CONTROL_RANGES_RAD, JOINT_NAMES, JOINT_RANGES_RAD,
    So101PlantError, TIMESTEP_NANOSECONDS, TIMESTEP_SECONDS,
    mock::{validate_state, validate_targets},
};

/// Headless `MuJoCo` plant backed by the externally pinned Menagerie model.
pub struct MujocoSo101Plant {
    id: &'static str,
    data: MjData<Box<MjModel>>,
    pending_control: Option<[f64; ACTUATOR_COUNT]>,
}

impl std::fmt::Debug for MujocoSo101Plant {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MujocoSo101Plant")
            .field("id", &self.id)
            .field("time", &self.data.time())
            .finish_non_exhaustive()
    }
}

impl MujocoSo101Plant {
    /// Loads `scene.xml` from `HEFAOS_SO101_MODEL_DIR`.
    ///
    /// # Errors
    ///
    /// Returns [`So101PlantError`] when the environment, model, or pinned model
    /// invariants are invalid.
    pub fn from_env() -> Result<Self, So101PlantError> {
        let directory =
            env::var_os("HEFAOS_SO101_MODEL_DIR").ok_or(So101PlantError::MissingModelDirectory)?;
        Self::from_model_dir(directory)
    }

    /// Loads the pinned SO-101 `scene.xml` from an external model directory.
    ///
    /// # Errors
    ///
    /// Returns [`So101PlantError`] when the model cannot be loaded or does not
    /// match the locked SO-101 dimensions, ordering, ranges, and timestep.
    pub fn from_model_dir(directory: impl AsRef<Path>) -> Result<Self, So101PlantError> {
        validate_execution_files(directory.as_ref())?;
        let scene = directory.as_ref().join("scene.xml");
        if !scene.is_file() {
            return Err(So101PlantError::MissingModelFile { path: scene });
        }

        let model = Box::new(MjModel::from_xml(&scene)?);
        validate_model(&model)?;
        let mut data = MjData::new(model);
        data.reset();
        data.forward();
        validate_simulation_values(&data)?;

        Ok(Self {
            id: "so101-mujoco",
            data,
            pending_control: None,
        })
    }

    pub(crate) const fn plant_id(&self) -> &'static str {
        self.id
    }

    /// Resets `MuJoCo` state and controller targets from the versioned contract.
    ///
    /// # Errors
    ///
    /// Returns [`So101PlantError`] for invalid state or non-finite simulation data.
    pub fn reset_state(&mut self, initial: &So101StateV0) -> Result<(), So101PlantError> {
        self.reset_values(
            initial.arm_position_rad,
            initial.arm_velocity_rad_s,
            initial.gripper_position,
            initial.gripper_velocity_per_s,
        )
    }

    /// Writes the last safety-authorized command to `MuJoCo`'s control vector.
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

    /// Advances `MuJoCo` by exactly one pinned 5 ms model period.
    ///
    /// # Errors
    ///
    /// Returns [`So101PlantError`] for another period or non-finite simulation data.
    pub fn advance_duration(&mut self, dt: DurationNs) -> Result<(), So101PlantError> {
        self.advance_ns(dt.get())
    }

    /// Returns the current `MuJoCo` state in contract units.
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

        let positions = model_positions(arm_position_rad, gripper_position);
        let velocities = model_velocities(arm_velocity_rad_s, gripper_velocity_per_s);
        self.data.reset();
        self.data.qpos_mut().copy_from_slice(&positions);
        self.data.qvel_mut().copy_from_slice(&velocities);
        self.data.ctrl_mut().copy_from_slice(&positions);
        self.data.forward();
        self.pending_control = None;
        validate_simulation_values(&self.data)
    }

    pub(crate) fn apply_values(
        &mut self,
        arm_position_rad: [f64; ARM_JOINT_COUNT],
        gripper_position: f64,
    ) -> Result<(), So101PlantError> {
        validate_targets(&arm_position_rad, gripper_position)?;
        let positions = model_positions(arm_position_rad, gripper_position);
        self.pending_control = Some(positions);
        Ok(())
    }

    pub(crate) fn advance_ns(&mut self, dt_ns: u64) -> Result<(), So101PlantError> {
        if dt_ns != TIMESTEP_NANOSECONDS {
            return Err(So101PlantError::InvalidTimestep { actual_ns: dt_ns });
        }
        let control = self.pending_control.take().unwrap_or_else(|| {
            let mut hold = [0.0; ACTUATOR_COUNT];
            hold.copy_from_slice(self.data.qpos());
            hold
        });
        self.data.ctrl_mut().copy_from_slice(&control);
        self.data.step();
        validate_simulation_values(&self.data)
    }

    pub(crate) fn state_values(
        &self,
    ) -> ([f64; ARM_JOINT_COUNT], [f64; ARM_JOINT_COUNT], f64, f64) {
        let mut arm_position_rad = [0.0; ARM_JOINT_COUNT];
        arm_position_rad.copy_from_slice(&self.data.qpos()[..ARM_JOINT_COUNT]);
        let mut arm_velocity_rad_s = [0.0; ARM_JOINT_COUNT];
        arm_velocity_rad_s.copy_from_slice(&self.data.qvel()[..ARM_JOINT_COUNT]);

        let [minimum, maximum] = CONTROL_RANGES_RAD[ARM_JOINT_COUNT];
        let range = maximum - minimum;
        let gripper_position =
            ((self.data.qpos()[ARM_JOINT_COUNT] - minimum) / range).clamp(0.0, 1.0);
        let gripper_velocity_per_s = self.data.qvel()[ARM_JOINT_COUNT] / range;

        (
            arm_position_rad,
            arm_velocity_rad_s,
            gripper_position,
            gripper_velocity_per_s,
        )
    }
}

fn validate_execution_files(directory: &Path) -> Result<(), So101PlantError> {
    for line in include_str!("../execution-files.sha256").lines() {
        let (expected, relative) = line
            .split_once("  ")
            .ok_or_else(|| invariant("invalid embedded execution-file manifest".to_owned()))?;
        let path = directory.join(relative);
        if !path.is_file() {
            return Err(So101PlantError::MissingModelFile { path });
        }
        let bytes = fs::read(&path).map_err(|source| So101PlantError::ModelFileRead {
            path: path.clone(),
            source,
        })?;
        let actual = format!("{:x}", Sha256::digest(bytes));
        if actual != expected {
            return Err(So101PlantError::ModelDigestMismatch {
                path,
                expected: expected.to_owned(),
                actual,
            });
        }
    }
    Ok(())
}

fn validate_model(model: &MjModel) -> Result<(), So101PlantError> {
    if model.njnt() != 6 || model.nq() != 6 || model.nv() != 6 || model.nu() != 6 {
        return Err(invariant(format!(
            "expected njnt=nq=nv=nu=6, got njnt={} nq={} nv={} nu={}",
            model.njnt(),
            model.nq(),
            model.nv(),
            model.nu()
        )));
    }

    if !model.opt().timestep.is_finite()
        || model.opt().timestep.to_bits() != TIMESTEP_SECONDS.to_bits()
    {
        return Err(invariant(format!(
            "expected an exact 0.005 second timestep, got {}",
            model.opt().timestep
        )));
    }

    for (id, expected_name) in JOINT_NAMES.iter().enumerate() {
        if model.id_to_name(MjtObj::mjOBJ_JOINT, id) != Some(*expected_name) {
            return Err(invariant(format!(
                "joint id {id} must be {expected_name:?}"
            )));
        }
        if model.id_to_name(MjtObj::mjOBJ_ACTUATOR, id) != Some(*expected_name) {
            return Err(invariant(format!(
                "actuator id {id} must be {expected_name:?}"
            )));
        }

        let expected_index = i32::try_from(id).expect("six indices fit in i32");
        if model.jnt_qposadr()[id] != expected_index || model.jnt_dofadr()[id] != expected_index {
            return Err(invariant(format!(
                "joint {expected_name} must use qpos and dof index {id}"
            )));
        }

        let joint_info = model
            .joint(expected_name)
            .ok_or_else(|| invariant(format!("missing joint {expected_name}")))?;
        let actuator_info = model
            .actuator(expected_name)
            .ok_or_else(|| invariant(format!("missing actuator {expected_name}")))?;
        if joint_info.id != id || actuator_info.id != id {
            return Err(invariant(format!(
                "named lookup for {expected_name} did not preserve id {id}"
            )));
        }

        let joint_view = joint_info.view(model);
        validate_range(
            expected_name,
            "joint",
            &joint_view.range,
            JOINT_RANGES_RAD[id],
        )?;

        let actuator_view = actuator_info.view(model);
        validate_range(
            expected_name,
            "control",
            &actuator_view.ctrlrange,
            CONTROL_RANGES_RAD[id],
        )?;
        if actuator_view.trnid[0] != expected_index {
            return Err(invariant(format!(
                "actuator {expected_name} must address joint id {id}"
            )));
        }
    }
    Ok(())
}

fn validate_range(
    name: &str,
    kind: &str,
    actual: &[f64],
    expected: [f64; 2],
) -> Result<(), So101PlantError> {
    if actual.len() != 2 || actual.iter().any(|value| !value.is_finite()) {
        return Err(invariant(format!(
            "{kind} range for {name} must contain two finite values"
        )));
    }
    if actual
        .iter()
        .zip(expected)
        .any(|(actual, expected)| (actual - expected).abs() > 1.0e-12)
    {
        return Err(invariant(format!(
            "{kind} range for {name} must be {expected:?}, got {actual:?}"
        )));
    }
    Ok(())
}

fn validate_simulation_values(data: &MjData<Box<MjModel>>) -> Result<(), So101PlantError> {
    for (channel, values) in [
        ("qpos", data.qpos()),
        ("qvel", data.qvel()),
        ("ctrl", data.ctrl()),
    ] {
        if values.iter().any(|value| !value.is_finite()) {
            return Err(So101PlantError::NonFiniteSimulation { channel });
        }
    }
    if !data.time().is_finite() {
        return Err(So101PlantError::NonFiniteSimulation { channel: "time" });
    }
    Ok(())
}

fn model_positions(
    arm_position_rad: [f64; ARM_JOINT_COUNT],
    gripper_position: f64,
) -> [f64; ACTUATOR_COUNT] {
    let [minimum, maximum] = CONTROL_RANGES_RAD[ARM_JOINT_COUNT];
    let mut positions = [0.0; ACTUATOR_COUNT];
    positions[..ARM_JOINT_COUNT].copy_from_slice(&arm_position_rad);
    positions[ARM_JOINT_COUNT] = minimum + gripper_position * (maximum - minimum);
    positions
}

fn model_velocities(
    arm_velocity_rad_s: [f64; ARM_JOINT_COUNT],
    gripper_velocity_per_s: f64,
) -> [f64; ACTUATOR_COUNT] {
    let [minimum, maximum] = CONTROL_RANGES_RAD[ARM_JOINT_COUNT];
    let mut velocities = [0.0; ACTUATOR_COUNT];
    velocities[..ARM_JOINT_COUNT].copy_from_slice(&arm_velocity_rad_s);
    velocities[ARM_JOINT_COUNT] = gripper_velocity_per_s * (maximum - minimum);
    velocities
}

fn invariant(message: String) -> So101PlantError {
    So101PlantError::ModelInvariant(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicitly_enabled_mujoco_loads_and_steps_the_pinned_model() {
        let mut plant = MujocoSo101Plant::from_env().unwrap_or_else(|error| {
            panic!("MuJoCo SO-101 verification was explicitly enabled but could not start: {error}")
        });
        plant
            .reset_values([0.0; ARM_JOINT_COUNT], [0.0; ARM_JOINT_COUNT], 0.0, 0.0)
            .unwrap();
        plant
            .apply_values([0.2, -0.25, 0.3, -0.2, 0.15], 0.4)
            .unwrap();
        plant.advance_ns(TIMESTEP_NANOSECONDS).unwrap();

        let (positions, velocities, gripper, gripper_velocity) = plant.state_values();
        assert!(positions.into_iter().all(f64::is_finite));
        assert!(velocities.into_iter().all(f64::is_finite));
        assert!(gripper.is_finite());
        assert!(gripper_velocity.is_finite());
    }

    #[test]
    fn modified_execution_asset_is_rejected_before_model_load() {
        let directory =
            std::env::temp_dir().join(format!("hefaos-so101-digest-test-{}", std::process::id()));
        let assets = directory.join("assets");
        fs::create_dir_all(&assets).expect("create temporary asset directory");
        fs::write(assets.join("base_motor_holder_so101_v1.stl"), b"modified")
            .expect("write modified asset");

        let error = validate_execution_files(&directory).expect_err("modified asset must fail");
        assert!(matches!(error, So101PlantError::ModelDigestMismatch { .. }));
        fs::remove_dir_all(directory).expect("remove temporary asset directory");
    }
}
