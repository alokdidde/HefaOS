//! Experimental, direct Copper v1.1.1 probe behind the testbench `Subject` seam.
//!
//! The harness supplies all inputs and owns the mock plant.  Copper owns the
//! source -> controller -> sink turn: the adapter observes the sink's typed
//! output and never calls a semantic subject outside that graph.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use cu29::clock::RobotClock;
use cu29::prelude::UnifiedLogType;
use cu29::prelude::app::CuSimApplication;
use cu29::prelude::memmap::{MmapSectionStorage, MmapUnifiedLoggerWrite};
use cu29::prelude::*;
use cu29::simulation::{CuTaskCallbackState, SimOverride, recorded_copperlist_timestamp};
use cu29_export::{copperlists_reader, keyframes_reader};
use cu29_unifiedlog::{UnifiedLogger, UnifiedLoggerBuilder, UnifiedLoggerIOReader};
use hefaos_testbench_contracts::{SubjectConfigV0, SubjectInputV0, SubjectOutputV0};
use hefaos_testbench_harness::{Subject, SubjectError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const COPPER_REVISION: &str = "fc2ebc4fe3583d1f433b75898ad7c9e4dd9e6af2";
pub const COPPER_SUBJECT_ID: &str = "copper-spike/v0";
/// The graph records only a bounded experimental run.  This is deliberately
/// small enough for the twelve-scenario corpus rather than a per-turn slab.
const LOG_SLAB_SIZE: Option<usize> = Some(64 * 1024 * 1024);
const MAX_LOG_FAMILY_BYTES: u64 = 64 * 1024 * 1024;

mod resources;
mod tasks;

pub use tasks::CopperTaskFault;

#[copper_runtime(config = "copperconfig.ron", sim_mode = true)]
struct CopperSpikeApp {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordedTurn {
    pub output: SubjectOutputV0,
}

/// Timing observations retained with Copper evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimingObservation {
    pub source: TimingObservationSource,
    pub iterations: u64,
    pub requested_period_ns: u64,
    pub observed_intervals_ns: Vec<u64>,
    pub missed_periods: Vec<u64>,
}

/// Identifies whether timing data came from direct host instrumentation or
/// Copper's generated rate-limited run loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimingObservationSource {
    /// One-turn `Subject` calls use a mock clock and cannot establish pacing.
    HostOneTurnCharacterization,
    /// Recorded `CopperList` timestamps produced while `CopperSpikeApp::run()`
    /// exercised the generated `LoopRateLimiter`.
    CopperRateLimitedRun,
}

/// Result of a bounded, rate-limited Copper timing execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PacedTimingRun {
    pub outputs: Vec<RecordedTurn>,
    pub timing: TimingObservation,
}

#[derive(Debug, Serialize, Deserialize)]
struct RetainedReplayManifest {
    scenario_name: String,
    scenario_sha256: String,
    invocation_id: String,
    passed: bool,
    copper_revision: String,
    live_log_base: String,
    live_segments: Vec<RecordedLogSegment>,
    provenance: EvidenceProvenance,
    timing: TimingObservation,
    outputs: Vec<RecordedTurn>,
}

#[derive(Debug, Serialize, Deserialize)]
struct EvidenceProvenance {
    copper_revision: String,
    rustc_version_verbose: String,
    operating_system: String,
    architecture: String,
    generated_config_sha256: String,
    named_resources: Vec<String>,
    log_slab_bytes: Option<usize>,
}

fn evidence_provenance() -> EvidenceProvenance {
    let rustc_version_verbose = Command::new("rustc")
        .arg("-Vv")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_else(|| "unavailable".to_owned());
    EvidenceProvenance {
        copper_revision: COPPER_REVISION.to_owned(),
        rustc_version_verbose,
        operating_system: std::env::consts::OS.to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
        generated_config_sha256: format!(
            "{:x}",
            Sha256::digest(include_bytes!("../copperconfig.ron"))
        ),
        named_resources: vec!["run.counter".to_owned()],
        log_slab_bytes: LOG_SLAB_SIZE,
    }
}

#[derive(Debug, Clone)]
pub struct RetainedReplayRequest {
    pub manifest_path: PathBuf,
    pub scenario_name: String,
    pub scenario_sha256: String,
    pub invocation_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RecordedLogSegment {
    name: String,
    sha256: String,
    bytes: u64,
}

fn sha256_file(path: &Path) -> Result<(String, u64), SubjectError> {
    let bytes = std::fs::read(path)
        .map_err(|error| contextual("read recorded log segment", path, error))?;
    let size = u64::try_from(bytes.len())
        .map_err(|error| contextual("measure recorded log segment", path, error))?;
    Ok((format!("{:x}", Sha256::digest(&bytes)), size))
}

fn log_family(log_base: &Path) -> Result<Vec<RecordedLogSegment>, SubjectError> {
    let parent = log_base
        .parent()
        .ok_or_else(|| SubjectError::Step("Copper log base has no parent".to_owned()))?;
    let prefix = format!(
        "{}_,",
        log_base.file_stem().unwrap_or_default().to_string_lossy()
    );
    let prefix = prefix.trim_end_matches(',');
    let mut paths = Vec::new();
    for entry in
        std::fs::read_dir(parent).map_err(|error| contextual("read log family", parent, error))?
    {
        let path = entry
            .map_err(|error| contextual("read log family entry", parent, error))?
            .path();
        if path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with(prefix))
            && path
                .extension()
                .is_some_and(|extension| extension == "copper")
        {
            paths.push(path);
        }
    }
    paths.sort();
    let mut total = 0_u64;
    let mut segments = Vec::with_capacity(paths.len());
    for path in paths {
        let (sha256, bytes) = sha256_file(&path)?;
        total = total.checked_add(bytes).ok_or_else(|| {
            SubjectError::Step("Copper log family byte count overflow".to_owned())
        })?;
        segments.push(RecordedLogSegment {
            name: path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| SubjectError::Step("non-UTF8 Copper log segment name".to_owned()))?
                .to_owned(),
            sha256,
            bytes,
        });
    }
    if segments.is_empty() || total > MAX_LOG_FAMILY_BYTES {
        return Err(SubjectError::Step(format!(
            "Copper log family violates bounded recording policy: {} segments, {total} bytes",
            segments.len()
        )));
    }
    Ok(segments)
}

fn contextual(phase: &str, path: &Path, error: impl std::fmt::Display) -> SubjectError {
    SubjectError::Step(format!("Copper {phase} [{}]: {error}", path.display()))
}

fn unique_run_dir() -> Result<PathBuf, SubjectError> {
    let root = std::env::var_os("HEFAOS_COPPER_EVIDENCE_DIR")
        .map_or_else(|| PathBuf::from("target/copper-spike"), PathBuf::from);
    std::fs::create_dir_all(&root)
        .map_err(|error| contextual("create evidence root", &root, error))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| contextual("read evidence timestamp", &root, error))?
        .as_nanos();
    for attempt in 0_u8..16 {
        let directory = root.join(format!("run-{}-{stamp}-{attempt}", std::process::id()));
        match std::fs::create_dir(&directory) {
            Ok(()) => return Ok(directory),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(contextual(
                    "create owned evidence directory",
                    &directory,
                    error,
                ));
            }
        }
    }
    Err(SubjectError::Reset(format!(
        "Copper could not allocate owned evidence directory under {}",
        root.display()
    )))
}

fn decode_output(bytes: &[u8], path: &Path) -> Result<SubjectOutputV0, SubjectError> {
    serde_json::from_slice(bytes).map_err(|error| contextual("decode sink output", path, error))
}

/// Run a complete source-injected sequence once, returning only Copper's sink
/// outputs.  The run records one bounded log family at `log_path`.
///
/// # Errors
///
/// Returns a contextual subject error if graph lifecycle, source injection,
/// recording, or sink-output decoding fails.
pub fn run_source_injected(
    config: &SubjectConfigV0,
    inputs: &[SubjectInputV0],
    log_path: &Path,
) -> Result<Vec<RecordedTurn>, SubjectError> {
    if log_path.exists() {
        return Err(SubjectError::Step(format!(
            "refusing to overwrite Copper recording {}",
            log_path.display()
        )));
    }
    let parent = log_path
        .parent()
        .ok_or_else(|| SubjectError::Step("Copper log path has no parent".to_owned()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| contextual("create recording directory", log_path, error))?;
    let config_json = serde_json::to_vec(config)
        .map_err(|error| contextual("encode reset config", log_path, error))?;
    let mut startup_callback = |step: <CopperSpikeApp as CuSimApplication<
        MmapSectionStorage,
        MmapUnifiedLoggerWrite,
    >>::Step<'_>|
     -> SimOverride {
        match step {
            default::SimStep::Source(CuTaskCallbackState::Process((), source)) => {
                source.clear_payload();
                SimOverride::ExecutedBySim
            }
            _ => SimOverride::ExecuteByRuntime,
        }
    };
    let (clock, _clock_mock) = RobotClock::mock();
    let mut app = CopperSpikeApp::builder()
        .with_clock(clock)
        .with_log_path(log_path, LOG_SLAB_SIZE)
        .map_err(|error| contextual("configure recording", log_path, error))?
        .with_sim_callback(&mut startup_callback)
        .build()
        .map_err(|error| contextual("build graph", log_path, error))?;
    app.start_all_tasks(&mut startup_callback)
        .map_err(|error| contextual("start graph", log_path, error))?;
    let mut recorded = Vec::with_capacity(inputs.len());
    for (index, input) in inputs.iter().enumerate() {
        let mut turn = tasks::WireTurn {
            config_json: (index == 0).then(|| config_json.clone()),
            input_json: serde_json::to_vec(input)
                .map_err(|error| contextual("encode source input", log_path, error))?,
            fault: None,
        };
        let mut output = None;
        let mut step_callback = |step: <CopperSpikeApp as CuSimApplication<
            MmapSectionStorage,
            MmapUnifiedLoggerWrite,
        >>::Step<'_>|
         -> SimOverride {
            match step {
                default::SimStep::Source(CuTaskCallbackState::Process((), source)) => {
                    source.set_payload(std::mem::take(&mut turn));
                    SimOverride::ExecutedBySim
                }
                default::SimStep::Sink(CuTaskCallbackState::Process(input, _)) => {
                    output = input.payload().cloned();
                    SimOverride::ExecuteByRuntime
                }
                _ => SimOverride::ExecuteByRuntime,
            }
        };
        app.run_one_iteration(&mut step_callback)
            .map_err(|error| contextual("process graph", log_path, error))?;
        let observed = output.take().ok_or_else(|| {
            SubjectError::Step(format!(
                "Copper sink emitted no output at turn {index} [{}]",
                log_path.display()
            ))
        })?;
        recorded.push(RecordedTurn {
            output: decode_output(&observed.output_json, log_path)?,
        });
    }
    app.stop_all_tasks(&mut startup_callback)
        .map_err(|error| contextual("stop graph", log_path, error))?;
    app.log_shutdown_completed()
        .map_err(|error| contextual("close recording", log_path, error))?;
    Ok(recorded)
}

/// Runs a bounded source-injected sequence through Copper's generated
/// rate-limited `run()` loop and derives cadence and missed-list evidence from
/// recorded `CopperList` timestamps and identifiers.
///
/// The callback deliberately terminates the generated loop by returning a
/// simulated source error after every requested turn was recorded. That error
/// is the bounded-run sentinel, not a successful control result; any other
/// termination, missing output, missing timestamp, or count mismatch fails
/// the evidence run closed.
///
/// # Errors
///
/// Returns an error when Copper cannot construct, pace, record, or close the
/// graph, or when recorded `CopperList` evidence is incomplete.
#[allow(clippy::too_many_lines)]
pub fn run_rate_limited_source_injected(
    config: &SubjectConfigV0,
    inputs: &[SubjectInputV0],
    log_path: &Path,
) -> Result<PacedTimingRun, SubjectError> {
    if inputs.is_empty() {
        return Err(SubjectError::Step(
            "Copper rate-limited timing requires at least one input".to_owned(),
        ));
    }
    if log_path.exists() {
        return Err(SubjectError::Step(format!(
            "refusing to overwrite Copper recording {}",
            log_path.display()
        )));
    }
    let parent = log_path
        .parent()
        .ok_or_else(|| SubjectError::Step("Copper log path has no parent".to_owned()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| contextual("create timing recording directory", log_path, error))?;
    let config_json = serde_json::to_vec(config)
        .map_err(|error| contextual("encode timing reset config", log_path, error))?;
    let mut startup_callback = |step: <CopperSpikeApp as CuSimApplication<
        MmapSectionStorage,
        MmapUnifiedLoggerWrite,
    >>::Step<'_>|
     -> SimOverride {
        match step {
            default::SimStep::Source(CuTaskCallbackState::Process((), source)) => {
                source.clear_payload();
                SimOverride::ExecutedBySim
            }
            _ => SimOverride::ExecuteByRuntime,
        }
    };
    let clock = RobotClock::new();
    let mut app = CopperSpikeApp::builder()
        .with_clock(clock)
        .with_log_path(log_path, LOG_SLAB_SIZE)
        .map_err(|error| contextual("configure timing recording", log_path, error))?
        .with_sim_callback(&mut startup_callback)
        .build()
        .map_err(|error| contextual("build timing graph", log_path, error))?;
    let mut next_input = 0_usize;
    let mut outputs = Vec::with_capacity(inputs.len());
    let termination = {
        let mut callback = |step: <CopperSpikeApp as CuSimApplication<
            MmapSectionStorage,
            MmapUnifiedLoggerWrite,
        >>::Step<'_>|
         -> SimOverride {
            match step {
                default::SimStep::Source(CuTaskCallbackState::Process((), source)) => {
                    let Some(input) = inputs.get(next_input) else {
                        return SimOverride::Errored("hefaos nominal timing completed".to_owned());
                    };
                    let turn = tasks::WireTurn {
                        config_json: (next_input == 0).then(|| config_json.clone()),
                        input_json: match serde_json::to_vec(input) {
                            Ok(encoded) => encoded,
                            Err(error) => {
                                return SimOverride::Errored(format!(
                                    "encode nominal timing input: {error}"
                                ));
                            }
                        },
                        fault: None,
                    };
                    next_input = next_input.saturating_add(1);
                    source.set_payload(turn);
                    SimOverride::ExecutedBySim
                }
                default::SimStep::Sink(CuTaskCallbackState::Process(input, _)) => {
                    let Some(wire) = input.payload() else {
                        return SimOverride::Errored(
                            "nominal timing sink emitted no output".to_owned(),
                        );
                    };
                    match decode_output(&wire.output_json, log_path) {
                        Ok(output) => outputs.push(RecordedTurn { output }),
                        Err(error) => return SimOverride::Errored(error.to_string()),
                    }
                    SimOverride::ExecuteByRuntime
                }
                _ => SimOverride::ExecuteByRuntime,
            }
        };
        app.run(&mut callback)
    };
    drop(app);

    match termination {
        Err(error)
            if error
                .to_string()
                .contains("hefaos nominal timing completed")
                && next_input == inputs.len() => {}
        Err(error) => return Err(contextual("run rate-limited timing graph", log_path, error)),
        Ok(()) => {
            return Err(SubjectError::Step(format!(
                "Copper rate-limited timing loop ended without its bounded-run sentinel [{}]",
                log_path.display()
            )));
        }
    }
    if next_input != inputs.len() || outputs.len() != inputs.len() {
        return Err(SubjectError::Step(format!(
            "Copper rate-limited timing recorded {} inputs and {} outputs, expected {} [{}]",
            next_input,
            outputs.len(),
            inputs.len(),
            log_path.display()
        )));
    }

    let UnifiedLogger::Read(log_reader) = UnifiedLoggerBuilder::new()
        .file_base_name(log_path)
        .build()
        .map_err(|error| contextual("open timing recording", log_path, error))?
    else {
        return Err(SubjectError::Step(format!(
            "Copper expected readable timing recording [{}]",
            log_path.display()
        )));
    };
    let mut reader = UnifiedLoggerIOReader::new(log_reader, UnifiedLogType::CopperList);
    let timestamps = copperlists_reader::<default::CuStampedDataSet>(&mut reader)
        .map(|copperlist| {
            let timestamp = recorded_copperlist_timestamp(&copperlist).ok_or_else(|| {
                SubjectError::Step(format!(
                    "Copper timing list {} has no recorded process timestamp [{}]",
                    copperlist.id,
                    log_path.display()
                ))
            })?;
            Ok((copperlist.id, timestamp.as_nanos()))
        })
        .collect::<Result<Vec<_>, SubjectError>>()?;
    if timestamps.len() != inputs.len() {
        return Err(SubjectError::Step(format!(
            "Copper timing recording contains {} CopperLists, expected {} [{}]",
            timestamps.len(),
            inputs.len(),
            log_path.display()
        )));
    }
    let requested_period_ns = 5_000_000_u64;
    let mut observed_intervals_ns = Vec::with_capacity(timestamps.len().saturating_sub(1));
    let mut missed_periods = Vec::new();
    for window in timestamps.windows(2) {
        let [(previous_id, previous), (current_id, current)] = window else {
            unreachable!("window size is fixed at two");
        };
        let interval = current.checked_sub(*previous).ok_or_else(|| {
            SubjectError::Step(format!(
                "Copper timing timestamp regressed from list {previous_id} to {current_id} [{}]",
                log_path.display()
            ))
        })?;
        observed_intervals_ns.push(interval);
        let next_expected_id = previous_id.checked_add(1).ok_or_else(|| {
            SubjectError::Step(format!(
                "Copper timing list identifier overflow after {previous_id} [{}]",
                log_path.display()
            ))
        })?;
        if *current_id < next_expected_id {
            return Err(SubjectError::Step(format!(
                "Copper timing list identifier regressed from {previous_id} to {current_id} [{}]",
                log_path.display()
            )));
        }
        // The recorded cadence may exceed the requested period without Copper
        // dropping a list. Retain cadence in `observed_intervals_ns`; only an
        // actual identifier gap is a missed `CopperList`.
        missed_periods.extend(next_expected_id..*current_id);
    }
    Ok(PacedTimingRun {
        outputs,
        timing: TimingObservation {
            source: TimingObservationSource::CopperRateLimitedRun,
            iterations: u64::try_from(timestamps.len()).unwrap_or(u64::MAX),
            requested_period_ns,
            observed_intervals_ns,
            missed_periods,
        },
    })
}

/// Exercise Copper's public exact-output recorded replay primitive against the
/// log generated by [`run_source_injected`].  This is log replay, not source
/// re-execution; any unreadable record or replay error fails closed.
///
/// # Errors
///
/// Returns a contextual subject error for unreadable logs, replay execution
/// failures, absent sink output, or a typed-output mismatch.
pub fn replay_recording(
    log_path: &Path,
    replay_log_path: &Path,
    expected: &[RecordedTurn],
) -> Result<Vec<RecordedTurn>, SubjectError> {
    let UnifiedLogger::Read(log_reader) = UnifiedLoggerBuilder::new()
        .file_base_name(log_path)
        .build()
        .map_err(|error| contextual("open recorded log", log_path, error))?
    else {
        return Err(SubjectError::Step(format!(
            "Copper expected readable recorded log [{}]",
            log_path.display()
        )));
    };
    let mut reader = UnifiedLoggerIOReader::new(log_reader, UnifiedLogType::CopperList);
    let (clock, _clock_mock) = RobotClock::mock();
    let mut inert = |_step: <CopperSpikeApp as CuSimApplication<
        MmapSectionStorage,
        MmapUnifiedLoggerWrite,
    >>::Step<'_>| SimOverride::ExecutedBySim;
    let mut app = CopperSpikeApp::builder()
        .with_clock(clock)
        .with_log_path(replay_log_path, LOG_SLAB_SIZE)
        .map_err(|error| contextual("configure replay recording", replay_log_path, error))?
        .with_sim_callback(&mut inert)
        .build()
        .map_err(|error| contextual("build replay graph", replay_log_path, error))?;
    app.start_all_tasks(&mut inert)
        .map_err(|error| contextual("start replay graph", replay_log_path, error))?;
    let mut replayed = Vec::new();
    for copperlist in copperlists_reader::<default::CuStampedDataSet>(&mut reader) {
        let mut sink_output = None;
        let mut callback = |step: <CopperSpikeApp as CuSimApplication<
            MmapSectionStorage,
            MmapUnifiedLoggerWrite,
        >>::Step<'_>| match step {
            default::SimStep::Sink(CuTaskCallbackState::Process(input, _)) => {
                sink_output = input.payload().cloned();
                SimOverride::ExecuteByRuntime
            }
            _ => default::recorded_replay_step(step, &copperlist),
        };
        app.run_one_iteration(&mut callback)
            .map_err(|error| contextual("replay recorded CopperList", log_path, error))?;
        let wire = sink_output.ok_or_else(|| {
            SubjectError::Step(format!(
                "Copper recorded replay emitted no sink output [{}]",
                log_path.display()
            ))
        })?;
        replayed.push(RecordedTurn {
            output: decode_output(&wire.output_json, log_path)?,
        });
    }
    app.stop_all_tasks(&mut inert)
        .map_err(|error| contextual("stop replay graph", replay_log_path, error))?;
    app.log_shutdown_completed()
        .map_err(|error| contextual("close replay recording", replay_log_path, error))?;
    if replayed.is_empty() {
        return Err(SubjectError::Step(format!(
            "Copper recording contains no CopperLists [{}]",
            log_path.display()
        )));
    }
    if replayed != expected {
        return Err(SubjectError::Step(format!(
            "Copper recorded replay output differs from the live graph [{}]",
            log_path.display()
        )));
    }
    Ok(replayed)
}

/// Replays retained finalized evidence without executing any source scenario.
///
/// # Errors
///
/// Returns an error if the manifest or log segment is missing/corrupt, or if
/// exact Copper replay differs from the recorded typed output sequence.
pub fn replay_retained(requests: &[RetainedReplayRequest]) -> Result<usize, SubjectError> {
    if requests.is_empty() {
        return Err(SubjectError::Step(
            "no retained Copper manifests".to_owned(),
        ));
    }
    for request in requests {
        let manifest_path = &request.manifest_path;
        let raw = std::fs::read(manifest_path)
            .map_err(|error| contextual("read retained manifest", manifest_path, error))?;
        let manifest: RetainedReplayManifest = serde_json::from_slice(&raw)
            .map_err(|error| contextual("decode retained manifest", manifest_path, error))?;
        if !manifest.passed
            || manifest.scenario_name != request.scenario_name
            || manifest.scenario_sha256 != request.scenario_sha256
            || manifest.invocation_id != request.invocation_id
        {
            return Err(SubjectError::Step(format!(
                "retained run identity, invocation, or verdict does not match corpus entry [{}]",
                manifest_path.display()
            )));
        }
        if manifest.copper_revision != COPPER_REVISION {
            return Err(SubjectError::Step(format!(
                "retained manifest has Copper revision {} not {COPPER_REVISION} [{}]",
                manifest.copper_revision,
                manifest_path.display()
            )));
        }
        let parent = manifest_path
            .parent()
            .ok_or_else(|| SubjectError::Step("retained manifest has no parent".to_owned()))?;
        let live = parent.join(&manifest.live_log_base);
        let segments = log_family(&live)?;
        if segments.len() != manifest.live_segments.len()
            || segments
                .iter()
                .zip(&manifest.live_segments)
                .any(|(actual, expected)| {
                    actual.name != expected.name
                        || actual.sha256 != expected.sha256
                        || actual.bytes != expected.bytes
                })
        {
            return Err(SubjectError::Step(format!(
                "retained log family digest/size mismatch [{}]",
                live.display()
            )));
        }
        let restored_keyframes =
            restore_recorded_keyframes(&live, &parent.join("retained-keyframe-restore.copper"))?;
        let expected_keyframes = usize::try_from(manifest.timing.iterations).map_err(|error| {
            SubjectError::Step(format!(
                "retained Copper iteration count does not fit usize: {error} [{}]",
                live.display()
            ))
        })?;
        if restored_keyframes != expected_keyframes {
            return Err(SubjectError::Step(format!(
                "retained Copper keyframe count {restored_keyframes} does not cover {} recorded turns [{}]",
                manifest.timing.iterations,
                live.display()
            )));
        }
        replay_recording(
            &live,
            &parent.join("retained-replay.copper"),
            &manifest.outputs,
        )?;
    }
    Ok(requests.len())
}

/// Restores every recorded `FrozenTasks` keyframe into a fresh Copper graph.
///
/// # Errors
///
/// Returns an error when required keyframe evidence is absent or a task cannot
/// thaw its bounded checkpoint. This checks the actual Copper thaw path rather
/// than merely serializing a snapshot in isolation.
pub fn restore_recorded_keyframes(
    log_path: &Path,
    restore_log_path: &Path,
) -> Result<usize, SubjectError> {
    let UnifiedLogger::Read(log_reader) = UnifiedLoggerBuilder::new()
        .file_base_name(log_path)
        .build()
        .map_err(|error| contextual("open keyframe log", log_path, error))?
    else {
        return Err(SubjectError::Step(format!(
            "Copper expected readable keyframe log [{}]",
            log_path.display()
        )));
    };
    let mut reader = UnifiedLoggerIOReader::new(log_reader, UnifiedLogType::FrozenTasks);
    let keyframes = keyframes_reader(&mut reader).collect::<Vec<_>>();
    if keyframes.is_empty() {
        return Err(SubjectError::Step(format!(
            "Copper recording has no FrozenTasks keyframes [{}]",
            log_path.display()
        )));
    }
    let (clock, _clock_mock) = RobotClock::mock();
    let mut callback = |_step: <CopperSpikeApp as CuSimApplication<
        MmapSectionStorage,
        MmapUnifiedLoggerWrite,
    >>::Step<'_>| SimOverride::ExecutedBySim;
    let mut app = CopperSpikeApp::builder()
        .with_clock(clock)
        .with_log_path(restore_log_path, LOG_SLAB_SIZE)
        .map_err(|error| contextual("configure keyframe restore", restore_log_path, error))?
        .with_sim_callback(&mut callback)
        .build()
        .map_err(|error| contextual("build keyframe restore graph", restore_log_path, error))?;
    app.start_all_tasks(&mut callback)
        .map_err(|error| contextual("start keyframe restore graph", restore_log_path, error))?;
    for keyframe in &keyframes {
        <CopperSpikeApp as CuSimApplication<MmapSectionStorage, MmapUnifiedLoggerWrite>>::restore_keyframe(&mut app, keyframe)
            .map_err(|error| contextual("restore FrozenTasks keyframe", log_path, error))?;
    }
    app.stop_all_tasks(&mut callback)
        .map_err(|error| contextual("stop keyframe restore graph", restore_log_path, error))?;
    app.log_shutdown_completed()
        .map_err(|error| contextual("close keyframe restore", restore_log_path, error))?;
    Ok(keyframes.len())
}

/// Restore keyframe `N`, execute that keyframe's recorded `CopperList`, and require the observed
/// sink's typed output to equal the original live output.  Copper v1.1.1 captures a
/// keyframe before its identically numbered `CopperList` executes. This is stronger
/// than a thaw-only check: it proves that Copper's `Freezable` controller state
/// is usable for the next graph iteration.
///
/// # Errors
///
/// Returns an error for absent or unreadable keyframes, an invalid index,
/// lifecycle failure, absent sink output, or a typed continuation mismatch.
#[allow(clippy::too_many_lines)]
pub fn restore_keyframe_and_execute_next(
    log_path: &Path,
    restore_log_path: &Path,
    keyframe_index: usize,
    expected: &RecordedTurn,
) -> Result<RecordedTurn, SubjectError> {
    let UnifiedLogger::Read(log_reader) = UnifiedLoggerBuilder::new()
        .file_base_name(log_path)
        .build()
        .map_err(|error| contextual("open continuation keyframe log", log_path, error))?
    else {
        return Err(SubjectError::Step(format!(
            "Copper expected readable keyframe log [{}]",
            log_path.display()
        )));
    };
    let mut reader = UnifiedLoggerIOReader::new(log_reader, UnifiedLogType::FrozenTasks);
    let keyframe = keyframes_reader(&mut reader)
        .nth(keyframe_index)
        .ok_or_else(|| {
            SubjectError::Step(format!(
                "Copper recording has no keyframe {keyframe_index} [{}]",
                log_path.display()
            ))
        })?;
    let UnifiedLogger::Read(log_reader) = UnifiedLoggerBuilder::new()
        .file_base_name(log_path)
        .build()
        .map_err(|error| contextual("open continuation CopperList log", log_path, error))?
    else {
        return Err(SubjectError::Step(format!(
            "Copper expected readable CopperList log [{}]",
            log_path.display()
        )));
    };
    let mut reader = UnifiedLoggerIOReader::new(log_reader, UnifiedLogType::CopperList);
    let recorded_copperlist = copperlists_reader::<default::CuStampedDataSet>(&mut reader)
        .find(|copperlist| copperlist.id == keyframe.culistid)
        .ok_or_else(|| {
            SubjectError::Step(format!(
                "Copper recording has no CopperList for keyframe {keyframe_index} [{}]",
                log_path.display()
            ))
        })?;
    let (clock, _clock_mock) = RobotClock::mock();
    let mut startup_callback = |step: <CopperSpikeApp as CuSimApplication<
        MmapSectionStorage,
        MmapUnifiedLoggerWrite,
    >>::Step<'_>|
     -> SimOverride {
        match step {
            default::SimStep::Source(CuTaskCallbackState::Process((), source)) => {
                source.clear_payload();
                SimOverride::ExecutedBySim
            }
            _ => SimOverride::ExecuteByRuntime,
        }
    };
    let mut app = CopperSpikeApp::builder()
        .with_clock(clock)
        .with_log_path(restore_log_path, LOG_SLAB_SIZE)
        .map_err(|error| contextual("configure keyframe continuation", restore_log_path, error))?
        .with_sim_callback(&mut startup_callback)
        .build()
        .map_err(|error| {
            contextual("build keyframe continuation graph", restore_log_path, error)
        })?;
    app.start_all_tasks(&mut startup_callback)
        .map_err(|error| {
            contextual("start keyframe continuation graph", restore_log_path, error)
        })?;
    <CopperSpikeApp as CuSimApplication<MmapSectionStorage, MmapUnifiedLoggerWrite>>::restore_keyframe(
        &mut app, &keyframe,
    )
    .map_err(|error| contextual("restore continuation keyframe", log_path, error))?;
    let mut observed = None;
    let mut callback = |step: <CopperSpikeApp as CuSimApplication<
        MmapSectionStorage,
        MmapUnifiedLoggerWrite,
    >>::Step<'_>|
     -> SimOverride {
        match step {
            default::SimStep::Sink(CuTaskCallbackState::Process(input, _)) => {
                observed = input.payload().cloned();
                SimOverride::ExecuteByRuntime
            }
            // This injects the recorded external source input but leaves the
            // regular controller executing. `recorded_replay_step` would copy
            // the controller output and cannot prove restored task state.
            _ => default::recorded_debug_replay_step(step, &recorded_copperlist),
        }
    };
    app.run_one_iteration(&mut callback)
        .map_err(|error| contextual("execute keyframe continuation", log_path, error))?;
    app.stop_all_tasks(&mut startup_callback)
        .map_err(|error| contextual("stop keyframe continuation graph", restore_log_path, error))?;
    app.log_shutdown_completed()
        .map_err(|error| contextual("close keyframe continuation", restore_log_path, error))?;
    let wire = observed.ok_or_else(|| {
        SubjectError::Step(format!(
            "Copper keyframe continuation emitted no sink output [{}]",
            log_path.display()
        ))
    })?;
    let actual = RecordedTurn {
        output: decode_output(&wire.output_json, log_path)?,
    };
    if actual != *expected {
        return Err(SubjectError::Step(format!(
            "Copper keyframe {keyframe_index} continuation differs from live typed output [{}]",
            log_path.display()
        )));
    }
    Ok(actual)
}

struct ActiveRun {
    app: CopperSpikeApp,
    log_path: PathBuf,
    config_json: Vec<u8>,
    sent_config: bool,
    turns: u64,
    outputs: Vec<RecordedTurn>,
    last_step_started: Option<Instant>,
    observed_intervals: Vec<Duration>,
    injected_fault: Option<(u64, CopperTaskFault)>,
    terminal_fault: bool,
}

/// Subject adapter with one static graph and one bounded recording family per
/// reset.  `step` runs exactly one Copper iteration; it never replays a source
/// prefix or creates a per-turn application.
pub struct CopperSubject {
    config: Option<SubjectConfigV0>,
    run_dir: Option<PathBuf>,
    active: Option<ActiveRun>,
    injected_fault: Option<(u64, CopperTaskFault)>,
}

impl Default for CopperSubject {
    fn default() -> Self {
        Self::new()
    }
}
impl CopperSubject {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            config: None,
            run_dir: None,
            active: None,
            injected_fault: None,
        }
    }

    /// Creates a subject whose controller will fail in the real Copper task at
    /// `turn`. This exists solely to prove the generic harness's fail-closed
    /// treatment of Copper task errors and panics.
    #[must_use]
    pub const fn with_injected_task_fault(turn: u64, fault: CopperTaskFault) -> Self {
        Self {
            config: None,
            run_dir: None,
            active: None,
            injected_fault: Some((turn, fault)),
        }
    }

    /// Returns timing characterization from the active graph, if it has not
    /// been finalized yet.
    #[must_use]
    pub fn timing_observation(&self) -> Option<TimingObservation> {
        self.active.as_ref().map(|active| TimingObservation {
            source: TimingObservationSource::HostOneTurnCharacterization,
            iterations: active.turns,
            requested_period_ns: 5_000_000,
            observed_intervals_ns: active
                .observed_intervals
                .iter()
                .map(|interval| u64::try_from(interval.as_nanos()).unwrap_or(u64::MAX))
                .collect(),
            // Direct `run_one_iteration` calls do not exercise Copper's
            // rate-limiter, so they cannot make a Copper missed-list claim.
            missed_periods: Vec::new(),
        })
    }

    fn stop_active(&mut self) -> Result<(), SubjectError> {
        let Some(mut active) = self.active.take() else {
            return Ok(());
        };
        let mut callback = |_step: <CopperSpikeApp as CuSimApplication<
            MmapSectionStorage,
            MmapUnifiedLoggerWrite,
        >>::Step<'_>| SimOverride::ExecutedBySim;
        active
            .app
            .stop_all_tasks(&mut callback)
            .map_err(|error| contextual("stop reset graph", &active.log_path, error))?;
        active
            .app
            .log_shutdown_completed()
            .map_err(|error| contextual("close reset recording", &active.log_path, error))
    }
}

impl Subject for CopperSubject {
    fn id(&self) -> &'static str {
        COPPER_SUBJECT_ID
    }
    fn reset(&mut self, config: &SubjectConfigV0) -> Result<(), SubjectError> {
        // A rejected reset must terminalize the prior graph first. Otherwise a
        // caller could send an invalid configuration and then keep stepping
        // under the preceding configuration.
        self.stop_active()?;
        self.config = None;
        self.run_dir = None;
        // Preserve the existing Subject reset contract synchronously.
        hefaos_testbench_reference::ReferenceSubject::new()
            .reset(config)
            .map_err(|error| {
                SubjectError::Reset(format!("Copper configuration rejected: {error}"))
            })?;
        self.stop_active()?;
        let run_dir = unique_run_dir()?;
        let log_path = run_dir.join("live.copper");
        let config_json = serde_json::to_vec(config)
            .map_err(|error| contextual("encode reset config", &log_path, error))?;
        let (clock, _clock_mock) = RobotClock::mock();
        let mut startup_callback = |step: <CopperSpikeApp as CuSimApplication<
            MmapSectionStorage,
            MmapUnifiedLoggerWrite,
        >>::Step<'_>| match step {
            default::SimStep::Source(CuTaskCallbackState::Process((), output)) => {
                output.clear_payload();
                SimOverride::ExecutedBySim
            }
            _ => SimOverride::ExecuteByRuntime,
        };
        let mut app = CopperSpikeApp::builder()
            .with_clock(clock)
            .with_log_path(&log_path, LOG_SLAB_SIZE)
            .map_err(|error| contextual("configure reset graph", &log_path, error))?
            .with_sim_callback(&mut startup_callback)
            .build()
            .map_err(|error| contextual("build reset graph", &log_path, error))?;
        app.start_all_tasks(&mut startup_callback)
            .map_err(|error| contextual("start reset graph", &log_path, error))?;
        self.config = Some(config.clone());
        self.run_dir = Some(run_dir);
        self.active = Some(ActiveRun {
            app,
            log_path,
            config_json,
            sent_config: false,
            turns: 0,
            outputs: Vec::new(),
            last_step_started: None,
            observed_intervals: Vec::new(),
            injected_fault: self.injected_fault,
            terminal_fault: false,
        });
        Ok(())
    }
    fn step(&mut self, input: &SubjectInputV0) -> Result<SubjectOutputV0, SubjectError> {
        let active = self.active.as_mut().ok_or_else(|| {
            SubjectError::Step("Copper subject must be reset before stepping".to_owned())
        })?;
        if active.terminal_fault {
            return Err(SubjectError::Step(
                "Copper graph is terminal after a task failure; reset is required".to_owned(),
            ));
        }
        let started = Instant::now();
        if let Some(previous) = active.last_step_started.replace(started) {
            active.observed_intervals.push(previous.elapsed());
        }
        let injected_fault = active
            .injected_fault
            .filter(|(turn, _)| *turn == active.turns)
            .map(|(_, fault)| fault);
        // Do not allow a real task error or unwind to leave this application
        // runnable. The generic harness will convert this turn into a terminal
        // SubjectFault, and every later turn fails closed without re-entering
        // Copper.
        if injected_fault.is_some() {
            active.terminal_fault = true;
            active.injected_fault = None;
        }
        let turn = tasks::WireTurn {
            config_json: (!active.sent_config).then(|| active.config_json.clone()),
            input_json: serde_json::to_vec(input)
                .map_err(|error| contextual("encode source input", &active.log_path, error))?,
            fault: injected_fault,
        };
        let mut source = Some(turn.clone());
        let mut observed = None;
        let mut callback = |step: <CopperSpikeApp as CuSimApplication<
            MmapSectionStorage,
            MmapUnifiedLoggerWrite,
        >>::Step<'_>| match step {
            default::SimStep::Source(CuTaskCallbackState::Process((), output)) => {
                if let Some(turn) = source.take() {
                    output.set_payload(turn);
                } else {
                    output.clear_payload();
                }
                SimOverride::ExecutedBySim
            }
            default::SimStep::Sink(CuTaskCallbackState::Process(input, _)) => {
                observed = input.payload().cloned();
                SimOverride::ExecuteByRuntime
            }
            _ => SimOverride::ExecuteByRuntime,
        };
        active
            .app
            .run_one_iteration(&mut callback)
            .map_err(|error| contextual("process graph", &active.log_path, error))?;
        active.sent_config = true;
        active.turns = active.turns.saturating_add(1);
        let wire = observed.ok_or_else(|| {
            SubjectError::Step(format!(
                "Copper sink emitted no output at turn {} [{}]",
                active.turns,
                active.log_path.display()
            ))
        })?;
        let output = decode_output(&wire.output_json, &active.log_path)?;
        active.outputs.push(RecordedTurn {
            output: output.clone(),
        });
        Ok(output)
    }
}

impl CopperSubject {
    /// Flush the reset-owned recording, then prove its exact Copper log replay.
    /// Call this after a complete corpus run; failure makes the run unusable as
    /// replay evidence.
    ///
    /// # Errors
    ///
    /// Returns a contextual subject error if the graph cannot stop/flush or
    /// its recorded Copper replay differs from the live graph outputs.
    pub fn finalize(
        &mut self,
        scenario_name: &str,
        scenario_sha256: &str,
        invocation_id: &str,
    ) -> Result<PathBuf, SubjectError> {
        let Some(active) = self.active.take() else {
            return Err(SubjectError::Step(
                "Copper subject has no active run to finalize".to_owned(),
            ));
        };
        let ActiveRun {
            mut app,
            log_path,
            outputs,
            turns,
            observed_intervals,
            ..
        } = active;
        let replay_path = log_path.with_file_name("replay.copper");
        let mut callback = |_step: <CopperSpikeApp as CuSimApplication<
            MmapSectionStorage,
            MmapUnifiedLoggerWrite,
        >>::Step<'_>| SimOverride::ExecutedBySim;
        app.stop_all_tasks(&mut callback)
            .map_err(|error| contextual("stop final graph", &log_path, error))?;
        app.log_shutdown_completed()
            .map_err(|error| contextual("close final recording", &log_path, error))?;
        // The reader must not race the mmap logger's final owner.  The one-shot
        // path has the same lifetime boundary before it opens a reader.
        drop(app);
        replay_recording(&log_path, &replay_path, &outputs)?;
        let restored_keyframes = restore_recorded_keyframes(
            &log_path,
            &log_path.with_file_name("keyframe-restore.copper"),
        )?;
        if restored_keyframes != outputs.len() {
            return Err(SubjectError::Step(format!(
                "Copper required {} keyframes for {} recorded turns, restored {restored_keyframes} [{}]",
                outputs.len(),
                outputs.len(),
                log_path.display()
            )));
        }
        if let Some(expected) = outputs.get(1) {
            restore_keyframe_and_execute_next(
                &log_path,
                &log_path.with_file_name("keyframe-continuation.copper"),
                1,
                expected,
            )?;
        } else {
            return Err(SubjectError::Step(
                "Copper recording needs at least two turns for keyframe continuation".to_owned(),
            ));
        }
        let live_segments = log_family(&log_path)?;
        let manifest = RetainedReplayManifest {
            scenario_name: scenario_name.to_owned(),
            scenario_sha256: scenario_sha256.to_owned(),
            invocation_id: invocation_id.to_owned(),
            passed: true,
            copper_revision: COPPER_REVISION.to_owned(),
            live_log_base: "live.copper".to_owned(),
            live_segments,
            provenance: evidence_provenance(),
            timing: TimingObservation {
                source: TimingObservationSource::HostOneTurnCharacterization,
                iterations: turns,
                requested_period_ns: 5_000_000,
                observed_intervals_ns: observed_intervals
                    .iter()
                    .map(|interval| u64::try_from(interval.as_nanos()).unwrap_or(u64::MAX))
                    .collect(),
                missed_periods: Vec::new(),
            },
            outputs,
        };
        let manifest_path = log_path.with_file_name("manifest.json");
        let encoded = serde_json::to_vec_pretty(&manifest)
            .map_err(|error| contextual("encode retained manifest", &manifest_path, error))?;
        std::fs::write(&manifest_path, encoded)
            .map_err(|error| contextual("write retained manifest", &manifest_path, error))?;
        Ok(manifest_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hefaos_testbench_contracts::{ScenarioV0, Tick, VirtualTimeNs};
    use hefaos_testbench_harness::Runner;
    use hefaos_testbench_so101::MockSo101Plant;
    use std::sync::Mutex;

    // Copper v1.1.1 installs process-global logger state while constructing an
    // application. Unit tests therefore must not construct graphs in parallel.
    static COPPER_APP_TEST_LOCK: Mutex<()> = Mutex::new(());

    struct TestCwd {
        previous: std::path::PathBuf,
    }

    impl TestCwd {
        fn enter() -> Self {
            let previous = std::env::current_dir().expect("test current directory");
            let directory = previous.join("target/copper-spike-test-crash-reports");
            std::fs::create_dir_all(&directory).expect("test crash-report directory");
            std::env::set_current_dir(&directory).expect("enter test crash-report directory");
            Self { previous }
        }
    }

    impl Drop for TestCwd {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.previous).expect("restore test current directory");
        }
    }
    fn scenario() -> ScenarioV0 {
        serde_json::from_str(include_str!(
            "../../scenarios/v0/nominal_tracking.scenario.json"
        ))
        .expect("committed scenario")
    }
    #[test]
    fn source_simulation_uses_the_graph_output_and_log_replay() {
        let _guard = COPPER_APP_TEST_LOCK.lock().expect("test lock");
        let _cwd = TestCwd::enter();
        let scenario = scenario();
        let config = scenario.subject_config();
        let input = SubjectInputV0 {
            tick: Tick(0),
            time_ns: VirtualTimeNs(0),
            setpoint: None,
            sensor: None,
            safety_status: None,
            proposal_fault: None,
        };
        let dir = unique_run_dir().expect("owned evidence directory");
        let live = dir.join("live.copper");
        let output = run_source_injected(&config, &[input], &live).expect("live graph run");
        assert_eq!(
            replay_recording(&live, &dir.join("replay.copper"), &output).expect("recorded replay"),
            output
        );
        assert_eq!(output.len(), 1);
    }

    #[test]
    fn finalized_subject_restores_a_keyframe_and_executes_the_following_turn() {
        let _guard = COPPER_APP_TEST_LOCK.lock().expect("test lock");
        let _cwd = TestCwd::enter();
        let scenario = scenario();
        let config = scenario.subject_config();
        let inputs = [
            SubjectInputV0 {
                tick: Tick(0),
                time_ns: VirtualTimeNs(0),
                setpoint: None,
                sensor: None,
                safety_status: None,
                proposal_fault: None,
            },
            SubjectInputV0 {
                tick: Tick(1),
                time_ns: VirtualTimeNs(5_000_000),
                setpoint: None,
                sensor: None,
                safety_status: None,
                proposal_fault: None,
            },
            SubjectInputV0 {
                tick: Tick(2),
                time_ns: VirtualTimeNs(10_000_000),
                setpoint: None,
                sensor: None,
                safety_status: None,
                proposal_fault: None,
            },
            SubjectInputV0 {
                tick: Tick(3),
                time_ns: VirtualTimeNs(15_000_000),
                setpoint: None,
                sensor: None,
                safety_status: None,
                proposal_fault: None,
            },
        ];
        let dir = unique_run_dir().expect("owned evidence directory");
        let live = dir.join("live.copper");
        let output = run_source_injected(&config, &inputs, &live).expect("live graph run");
        assert_eq!(
            restore_keyframe_and_execute_next(
                &live,
                &dir.join("keyframe-continuation.copper"),
                1,
                &output[1],
            )
            .expect("keyframe continuation"),
            output[2]
        );
        let mut forged_expected = output[1].clone();
        forged_expected.output.lifecycle =
            hefaos_testbench_contracts::SubjectLifecycleV0::Faulted {
                reason: "forged continuation output".to_owned(),
            };
        assert!(
            restore_keyframe_and_execute_next(
                &live,
                &dir.join("keyframe-continuation-mismatch.copper"),
                1,
                &forged_expected,
            )
            .is_err()
        );
    }

    #[test]
    fn real_copper_task_errors_and_panics_fail_closed_and_are_nonreplayable() {
        let _guard = COPPER_APP_TEST_LOCK.lock().expect("test lock");
        let _cwd = TestCwd::enter();
        for fault in [CopperTaskFault::Error, CopperTaskFault::Panic] {
            let scenario = scenario();
            let outcome = Runner::new(
                CopperSubject::with_injected_task_fault(0, fault),
                MockSo101Plant::from_scenario(&scenario).expect("mock plant"),
            )
            .run(&scenario, &"0".repeat(64))
            .expect("harness converts Copper task fault to trace evidence");
            assert!(!outcome.trace.summary.replayable);
            assert!(!outcome.verdict.passed);
            assert_eq!(outcome.trace.summary.authorized_ticks, 0);
            assert!(matches!(
                outcome.trace.records[0]
                    .safety_controller_after
                    .controller_state,
                hefaos_testbench_contracts::SafetyControllerStateV0::Tripped {
                    reason: hefaos_testbench_contracts::SafetyTripReasonV0::SubjectFault
                }
            ));
        }
    }

    #[test]
    fn rejected_reset_terminalizes_the_prior_graph() {
        let _guard = COPPER_APP_TEST_LOCK.lock().expect("test lock");
        let _cwd = TestCwd::enter();
        let scenario = scenario();
        let mut subject = CopperSubject::new();
        subject
            .reset(&scenario.subject_config())
            .expect("reset valid Copper graph");
        let input = SubjectInputV0 {
            tick: Tick(0),
            time_ns: VirtualTimeNs(0),
            setpoint: None,
            sensor: None,
            safety_status: None,
            proposal_fault: None,
        };
        subject.step(&input).expect("old graph produces a turn");
        let mut invalid = scenario.subject_config();
        invalid.schema_version = invalid.schema_version.saturating_add(1);
        assert!(
            subject.reset(&invalid).is_err(),
            "invalid reset is rejected"
        );
        assert!(
            subject.step(&input).is_err(),
            "a rejected reset must not leave the old graph runnable"
        );
    }
}
