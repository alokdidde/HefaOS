//! Deterministic SO-101 plants for the `HefaOS` conformance testbench.
//!
//! The mock plant is always available. The native `MuJoCo` plant is opt-in via
//! the `mujoco` feature and loads the separately pinned Menagerie directory at
//! runtime rather than embedding its mesh assets in this crate.

mod error;
mod mock;
mod model;
#[cfg(feature = "mujoco")]
mod mujoco;
mod plant;

pub use error::So101PlantError;
pub use mock::MockSo101Plant;
#[cfg(feature = "mujoco")]
pub use mujoco::MujocoSo101Plant;

pub use model::{
    ACTUATOR_COUNT, ARM_JOINT_COUNT, CONTROL_RANGES_RAD, JOINT_NAMES, JOINT_RANGES_RAD,
    TIMESTEP_NANOSECONDS, TIMESTEP_SECONDS,
};
