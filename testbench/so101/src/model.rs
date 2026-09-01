//! Pinned SO-101 model invariants shared by both plant implementations.

/// Number of independently commanded arm joints, excluding the gripper.
pub const ARM_JOINT_COUNT: usize = 5;

/// Number of position-controlled actuators in the pinned model.
pub const ACTUATOR_COUNT: usize = 6;

/// Simulation period declared by the pinned MJCF model.
pub const TIMESTEP_SECONDS: f64 = 0.005;

/// Simulation period in the testbench's integer time unit.
pub const TIMESTEP_NANOSECONDS: u64 = 5_000_000;

/// Joint and actuator names in `MuJoCo` id/control-vector order.
pub const JOINT_NAMES: [&str; ACTUATOR_COUNT] = [
    "shoulder_pan",
    "shoulder_lift",
    "elbow_flex",
    "wrist_flex",
    "wrist_roll",
    "gripper",
];

/// Hinge limits compiled from `robotstudio_so101/so101.xml`, in radians.
pub const JOINT_RANGES_RAD: [[f64; 2]; ACTUATOR_COUNT] = [
    [-1.91986, 1.91986],
    [-1.745_329_3, 1.745_329_3],
    [-1.69, 1.69],
    [-1.658_063, 1.658_063],
    [-2.743_847_3, 2.743_847_3],
    [-0.174_533, 1.745_329_2],
];

/// Position-actuator control limits compiled from the pinned model, in radians.
pub const CONTROL_RANGES_RAD: [[f64; 2]; ACTUATOR_COUNT] = [
    [-1.91986, 1.91986],
    [-1.74533, 1.74533],
    [-1.69, 1.69],
    [-1.65806, 1.65806],
    [-2.74385, 2.84121],
    [-0.17453, 1.74533],
];

pub(crate) fn values_are_finite<const N: usize>(values: &[f64; N]) -> bool {
    values.iter().all(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hefaos_testbench_contracts::PINNED_SO101_MODEL_DIGEST;

    #[test]
    fn pinned_metadata_is_finite_and_ordered() {
        assert_eq!(JOINT_NAMES.len(), ACTUATOR_COUNT);
        assert!(TIMESTEP_SECONDS.is_finite());
        assert_eq!(TIMESTEP_SECONDS.to_bits(), 0.005_f64.to_bits());
        assert_eq!(TIMESTEP_NANOSECONDS, 5_000_000);

        for range in JOINT_RANGES_RAD.into_iter().chain(CONTROL_RANGES_RAD) {
            assert!(range.into_iter().all(f64::is_finite));
            assert!(range[0] < range[1]);
        }

        let model_hash = include_str!("../execution-files.sha256")
            .lines()
            .find_map(|line| line.strip_suffix("  so101.xml"))
            .expect("execution manifest contains so101.xml");
        assert_eq!(PINNED_SO101_MODEL_DIGEST, format!("sha256:{model_hash}"));
    }
}
