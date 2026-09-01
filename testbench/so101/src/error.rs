use thiserror::Error;

/// Errors detected at the SO-101 plant boundary.
#[derive(Debug, Error)]
pub enum So101PlantError {
    #[error("{channel} contains a non-finite value")]
    NonFinite { channel: &'static str },

    #[error("{channel}[{index}]={value} is outside [{minimum}, {maximum}]")]
    OutOfRange {
        channel: &'static str,
        index: usize,
        value: f64,
        minimum: f64,
        maximum: f64,
    },

    #[error("SO-101 plants require exactly 5 ms (5000000 ns) per advance; got {actual_ns} ns")]
    InvalidTimestep { actual_ns: u64 },

    #[cfg(feature = "mujoco")]
    #[error(
        "HEFAOS_SO101_MODEL_DIR is required when the `mujoco` feature is enabled; run testbench/tools/with-mujoco.sh"
    )]
    MissingModelDirectory,

    #[cfg(feature = "mujoco")]
    #[error("SO-101 model file does not exist: {path}")]
    MissingModelFile { path: std::path::PathBuf },

    #[cfg(feature = "mujoco")]
    #[error("failed to read pinned SO-101 model file {path}: {source}")]
    ModelFileRead {
        path: std::path::PathBuf,
        source: std::io::Error,
    },

    #[cfg(feature = "mujoco")]
    #[error("pinned SO-101 model digest mismatch for {path}: expected {expected}, got {actual}")]
    ModelDigestMismatch {
        path: std::path::PathBuf,
        expected: String,
        actual: String,
    },

    #[cfg(feature = "mujoco")]
    #[error("failed to load the pinned SO-101 MuJoCo model: {0}")]
    ModelLoad(#[from] mujoco_rs::prelude::MjModelError),

    #[cfg(feature = "mujoco")]
    #[error("pinned SO-101 model invariant failed: {0}")]
    ModelInvariant(String),

    #[cfg(feature = "mujoco")]
    #[error("MuJoCo produced a non-finite {channel} value")]
    NonFiniteSimulation { channel: &'static str },
}
