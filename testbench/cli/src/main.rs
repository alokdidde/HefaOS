use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use hefaos_testbench_contracts::{BenchmarkReportV0, ScenarioV0, SemanticTraceV0};
#[cfg(test)]
use hefaos_testbench_harness::validate_trace_evidence;
use hefaos_testbench_harness::{
    Plant, RunOutcome, Runner, benchmark_report, compare_semantic_traces, replay_semantic_trace,
    semantic_trace_sha256, validate_scenario,
};
use hefaos_testbench_reference::{REFERENCE_SUBJECT_ID, ReferenceSubject};
use hefaos_testbench_so101::MockSo101Plant;
#[cfg(feature = "mujoco")]
use hefaos_testbench_so101::MujocoSo101Plant;
use serde_json::json;
use sha2::{Digest, Sha256};

#[derive(Debug, Parser)]
#[command(
    name = "hefaos-testbench",
    version,
    about = "Deterministic SO-101 conformance, replay, and benchmark bench"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List the versioned scenarios shipped with this checkout.
    List,
    /// Parse and validate a scenario without running a subject or plant.
    Validate {
        #[arg(value_name = "SCENARIO")]
        scenario: PathBuf,
    },
    /// Run one scenario and optionally write its semantic trace.
    Run {
        #[arg(value_name = "SCENARIO")]
        scenario: PathBuf,
        #[arg(long, value_enum, default_value_t = PlantKind::Mock)]
        plant: PlantKind,
        #[arg(long, value_name = "DIRECTORY")]
        model_dir: Option<PathBuf>,
        #[arg(long, value_name = "TRACE.json")]
        trace: Option<PathBuf>,
    },
    /// Run the same scenario twice and verify its declared replay contract.
    Replay {
        #[arg(value_name = "SCENARIO")]
        scenario: PathBuf,
        #[arg(long, value_enum, default_value_t = PlantKind::Mock)]
        plant: PlantKind,
        #[arg(long, value_name = "DIRECTORY")]
        model_dir: Option<PathBuf>,
    },
    /// Compare two normalized semantic traces.
    Compare {
        #[arg(value_name = "LEFT.json")]
        left: PathBuf,
        #[arg(value_name = "RIGHT.json")]
        right: PathBuf,
    },
    /// Report host timing separately from semantic pass/fail verdicts.
    Benchmark {
        #[arg(value_name = "SCENARIO")]
        scenario: PathBuf,
        #[arg(long, value_enum, default_value_t = PlantKind::Mock)]
        plant: PlantKind,
        #[arg(long, value_name = "DIRECTORY")]
        model_dir: Option<PathBuf>,
        #[arg(long, default_value_t = 100)]
        warmup: u64,
        #[arg(long, default_value_t = 1_000)]
        iterations: u64,
        #[arg(long, value_name = "REPORT.json")]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
enum PlantKind {
    #[default]
    Mock,
    Mujoco,
}

#[derive(Debug)]
struct LoadedScenario {
    scenario: ScenarioV0,
    sha256: String,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::List => list_scenarios(),
        Command::Validate { scenario } => validate_command(&scenario),
        Command::Run {
            scenario,
            plant,
            model_dir,
            trace,
        } => run_command(&scenario, plant, model_dir.as_deref(), trace.as_deref()),
        Command::Replay {
            scenario,
            plant,
            model_dir,
        } => replay_command(&scenario, plant, model_dir.as_deref()),
        Command::Compare { left, right } => compare_command(&left, &right),
        Command::Benchmark {
            scenario,
            plant,
            model_dir,
            warmup,
            iterations,
            output,
        } => benchmark_command(
            &scenario,
            plant,
            model_dir.as_deref(),
            warmup,
            iterations,
            output.as_deref(),
        ),
    }
}

fn list_scenarios() -> Result<()> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scenarios/v0");
    let mut paths: Vec<_> = fs::read_dir(&directory)
        .with_context(|| format!("read scenario directory {}", directory.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    paths.sort();
    for path in paths {
        println!("{}", path.display());
    }
    Ok(())
}

fn validate_command(path: &Path) -> Result<()> {
    let loaded = load_scenario(path)?;
    if let Err(failures) = validate_scenario(&loaded.scenario) {
        bail!("scenario validation failed:\n{}", failures.join("\n"));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
          "valid": true,
          "name": loaded.scenario.name,
          "sha256": loaded.sha256,
        }))?
    );
    Ok(())
}

fn run_command(
    path: &Path,
    plant_kind: PlantKind,
    model_dir: Option<&Path>,
    trace_path: Option<&Path>,
) -> Result<()> {
    let loaded = load_scenario(path)?;
    let outcome = run_selected(&loaded, plant_kind, model_dir)?;
    if let Some(trace_path) = trace_path {
        write_json(trace_path, &outcome.trace)?;
    }
    let digest = semantic_trace_sha256(&outcome.trace)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
          "passed": outcome.verdict.passed,
          "scenario": outcome.trace.scenario_name,
          "subject": outcome.trace.subject_id,
          "plant": outcome.trace.plant_id,
          "traceSha256": digest,
          "summary": outcome.trace.summary,
          "failures": outcome.verdict.failures,
        }))?
    );
    require_verdict(&outcome)
}

fn replay_command(path: &Path, plant_kind: PlantKind, model_dir: Option<&Path>) -> Result<()> {
    let loaded = load_scenario(path)?;
    let captured = run_selected(&loaded, plant_kind, model_dir)?;
    require_verdict(&captured)?;
    let comparison = replay_semantic_trace(&mut ReferenceSubject::new(), &captured.trace);
    if !comparison.equal {
        bail!(
            "replay comparison failed:\n{}",
            comparison.differences.join("\n")
        );
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
          "capturedInputReplayMatched": true,
          "scenario": captured.trace.scenario_name,
          "plant": captured.trace.plant_id,
          "traceSha256": semantic_trace_sha256(&captured.trace)?,
        }))?
    );
    Ok(())
}

fn compare_command(left_path: &Path, right_path: &Path) -> Result<()> {
    let left: SemanticTraceV0 = read_json(left_path)?;
    let right: SemanticTraceV0 = read_json(right_path)?;
    let comparison = compare_semantic_traces(&left, &right);
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
          "equal": comparison.equal,
          "differences": comparison.differences,
        }))?
    );
    if comparison.equal {
        Ok(())
    } else {
        bail!("semantic traces differ")
    }
}

fn benchmark_command(
    path: &Path,
    plant_kind: PlantKind,
    model_dir: Option<&Path>,
    warmup: u64,
    iterations: u64,
    output: Option<&Path>,
) -> Result<()> {
    if iterations == 0 {
        bail!("iterations must be positive");
    }
    let loaded = load_scenario(path)?;
    let report = match plant_kind {
        PlantKind::Mock => {
            let plant = MockSo101Plant::from_scenario(&loaded.scenario)?;
            benchmark_with(&loaded, plant, warmup, iterations)?
        }
        PlantKind::Mujoco => benchmark_mujoco(&loaded, model_dir, warmup, iterations)?,
    };
    if let Some(path) = output {
        write_json(path, &report)?;
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    if report.semantic_failures == 0 {
        Ok(())
    } else {
        bail!(
            "{} semantic benchmark iterations failed",
            report.semantic_failures
        )
    }
}

fn run_selected(
    loaded: &LoadedScenario,
    plant_kind: PlantKind,
    model_dir: Option<&Path>,
) -> Result<RunOutcome> {
    match plant_kind {
        PlantKind::Mock => {
            let plant = MockSo101Plant::from_scenario(&loaded.scenario)?;
            run_with(loaded, plant)
        }
        PlantKind::Mujoco => run_mujoco(loaded, model_dir),
    }
}

fn run_with<P: Plant>(loaded: &LoadedScenario, plant: P) -> Result<RunOutcome> {
    let mut runner = Runner::new(ReferenceSubject::new(), plant);
    runner
        .run(&loaded.scenario, &loaded.sha256)
        .context("run scenario")
}

#[cfg(feature = "mujoco")]
fn run_mujoco(loaded: &LoadedScenario, model_dir: Option<&Path>) -> Result<RunOutcome> {
    let plant = match model_dir {
        Some(directory) => MujocoSo101Plant::from_model_dir(directory)?,
        None => MujocoSo101Plant::from_env()?,
    };
    run_with(loaded, plant)
}

#[cfg(not(feature = "mujoco"))]
fn run_mujoco(_loaded: &LoadedScenario, _model_dir: Option<&Path>) -> Result<RunOutcome> {
    bail!(
        "MuJoCo was requested but this binary lacks the `mujoco` feature; use ./testbench/tools/with-mujoco.sh"
    )
}

fn benchmark_with<P: Plant>(
    loaded: &LoadedScenario,
    plant: P,
    warmup: u64,
    iterations: u64,
) -> Result<BenchmarkReportV0> {
    let plant_id = plant.id().to_owned();
    let mut runner = Runner::new(ReferenceSubject::new(), plant);
    for _ in 0..warmup {
        let outcome = runner.run(&loaded.scenario, &loaded.sha256)?;
        require_verdict(&outcome)?;
    }
    let mut semantic_failures = 0;
    let mut samples = Vec::new();
    for _ in 0..iterations {
        let outcome = runner.run(&loaded.scenario, &loaded.sha256)?;
        if !outcome.verdict.passed {
            semantic_failures += 1;
        }
        samples.extend(outcome.control_turn_latency_ns);
    }
    Ok(benchmark_report(
        &loaded.scenario,
        &loaded.sha256,
        REFERENCE_SUBJECT_ID,
        &plant_id,
        iterations,
        semantic_failures,
        &samples,
    ))
}

#[cfg(feature = "mujoco")]
fn benchmark_mujoco(
    loaded: &LoadedScenario,
    model_dir: Option<&Path>,
    warmup: u64,
    iterations: u64,
) -> Result<BenchmarkReportV0> {
    let plant = match model_dir {
        Some(directory) => MujocoSo101Plant::from_model_dir(directory)?,
        None => MujocoSo101Plant::from_env()?,
    };
    benchmark_with(loaded, plant, warmup, iterations)
}

#[cfg(not(feature = "mujoco"))]
fn benchmark_mujoco(
    _loaded: &LoadedScenario,
    _model_dir: Option<&Path>,
    _warmup: u64,
    _iterations: u64,
) -> Result<BenchmarkReportV0> {
    bail!(
        "MuJoCo was requested but this binary lacks the `mujoco` feature; use ./testbench/tools/with-mujoco.sh"
    )
}

fn load_scenario(path: &Path) -> Result<LoadedScenario> {
    let bytes = fs::read(path).with_context(|| format!("read scenario {}", path.display()))?;
    let scenario = serde_json::from_slice(&bytes)
        .with_context(|| format!("decode scenario {}", path.display()))?;
    Ok(LoadedScenario {
        scenario,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
    })
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("decode {}", path.display()))
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output directory {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes).with_context(|| format!("write {}", path.display()))
}

fn require_verdict(outcome: &RunOutcome) -> Result<()> {
    if outcome.verdict.passed {
        Ok(())
    } else {
        bail!(
            "scenario verdict failed:\n{}",
            outcome.verdict.failures.join("\n")
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use hefaos_testbench_contracts::{FaultKindV0, ProposalFaultV0};

    use super::*;

    fn scenario_paths() -> Vec<PathBuf> {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scenarios/v0");
        let mut paths: Vec<_> = fs::read_dir(directory)
            .expect("read scenario corpus")
            .map(|entry| entry.expect("read scenario entry").path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect();
        paths.sort();
        paths
    }

    fn run_mock(loaded: &LoadedScenario) -> RunOutcome {
        run_with(
            loaded,
            MockSo101Plant::from_scenario(&loaded.scenario).expect("construct mock plant"),
        )
        .expect("run scenario")
    }

    fn fault_branch(fault: &FaultKindV0) -> (&'static str, Option<&'static str>) {
        match fault {
            FaultKindV0::DropSensor => ("drop_sensor", None),
            FaultKindV0::InvalidateSensor { .. } => ("invalidate_sensor", None),
            FaultKindV0::OutOfRangeSensor => ("out_of_range_sensor", None),
            FaultKindV0::FutureSensor { .. } => ("future_sensor", None),
            FaultKindV0::OverAgeSensor { .. } => ("over_age_sensor", None),
            FaultKindV0::StaleSensor { .. } => ("stale_sensor", None),
            FaultKindV0::DuplicateSensor => ("duplicate_sensor", None),
            FaultKindV0::ReorderSensor => ("reorder_sensor", None),
            FaultKindV0::WrongSensorClockEpoch => ("wrong_sensor_clock_epoch", None),
            FaultKindV0::WrongSensorSourceEpoch => ("wrong_sensor_source_epoch", None),
            FaultKindV0::DropSetpoint => ("drop_setpoint", None),
            FaultKindV0::InvalidateSetpoint { .. } => ("invalidate_setpoint", None),
            FaultKindV0::OutOfRangeSetpoint => ("out_of_range_setpoint", None),
            FaultKindV0::FutureSetpoint { .. } => ("future_setpoint", None),
            FaultKindV0::OverAgeSetpoint { .. } => ("over_age_setpoint", None),
            FaultKindV0::StaleSetpoint { .. } => ("stale_setpoint", None),
            FaultKindV0::DuplicateSetpoint => ("duplicate_setpoint", None),
            FaultKindV0::ReorderSetpoint => ("reorder_setpoint", None),
            FaultKindV0::WrongSetpointClockEpoch => ("wrong_setpoint_clock_epoch", None),
            FaultKindV0::WrongSetpointSourceEpoch => ("wrong_setpoint_source_epoch", None),
            FaultKindV0::DropSafetyStatus => ("drop_safety_status", None),
            FaultKindV0::InvalidateSafetyStatus { .. } => ("invalidate_safety_status", None),
            FaultKindV0::OpenSafetyInterlock => ("open_safety_interlock", None),
            FaultKindV0::FutureSafetyStatus { .. } => ("future_safety_status", None),
            FaultKindV0::OverAgeSafetyStatus { .. } => ("over_age_safety_status", None),
            FaultKindV0::StaleSafetyStatus { .. } => ("stale_safety_status", None),
            FaultKindV0::DuplicateSafetyStatus => ("duplicate_safety_status", None),
            FaultKindV0::ReorderSafetyStatus => ("reorder_safety_status", None),
            FaultKindV0::WrongSafetyClockEpoch => ("wrong_safety_clock_epoch", None),
            FaultKindV0::WrongSafetySourceEpoch => ("wrong_safety_source_epoch", None),
            FaultKindV0::WrongSafetyPermitEpoch => ("wrong_safety_permit_epoch", None),
            FaultKindV0::RevokePermit => ("revoke_permit", None),
            FaultKindV0::EmergencyStop => ("emergency_stop", None),
            FaultKindV0::DriveFault => ("drive_fault", None),
            FaultKindV0::Proposal { fault } => (
                "proposal",
                Some(match fault {
                    ProposalFaultV0::Expired => "expired",
                    ProposalFaultV0::SourceSequenceMismatch => "source_sequence_mismatch",
                    ProposalFaultV0::NonFinite => "non_finite",
                    ProposalFaultV0::OutOfRange => "out_of_range",
                    ProposalFaultV0::TaskError => "task_error",
                }),
            ),
            FaultKindV0::DropIntent => ("drop_intent", None),
        }
    }

    #[test]
    fn corpus_exercises_every_declared_fault_branch() {
        let mut top_level = BTreeSet::new();
        let mut proposals = BTreeSet::new();
        for path in scenario_paths() {
            let loaded = load_scenario(&path).expect("load shipped scenario");
            for scheduled in &loaded.scenario.faults {
                let (branch, proposal) = fault_branch(&scheduled.fault);
                top_level.insert(branch);
                proposals.extend(proposal);
            }
        }

        assert_eq!(top_level.len(), 36, "every FaultKindV0 branch must ship");
        assert_eq!(
            proposals,
            BTreeSet::from([
                "expired",
                "non_finite",
                "out_of_range",
                "source_sequence_mismatch",
                "task_error",
            ])
        );
    }

    #[test]
    fn every_shipped_scenario_passes_captured_replay_and_exact_resimulation() {
        let paths = scenario_paths();
        assert!(
            paths.len() >= 5,
            "the conformance corpus must not disappear"
        );
        for path in paths {
            let loaded = load_scenario(&path).expect("load shipped scenario");
            validate_scenario(&loaded.scenario)
                .unwrap_or_else(|failures| panic!("{} is invalid: {failures:?}", path.display()));
            let first = run_mock(&loaded);
            assert!(
                first.verdict.passed,
                "{} failed: {:?}",
                path.display(),
                first.verdict.failures
            );
            validate_trace_evidence(&first.trace).expect("validate first trace evidence");
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("UTF-8 scenario filename")
                .replace(".scenario.json", ".semantic-trace.json");
            let golden_path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../goldens/v0")
                .join(file_name);
            let golden: SemanticTraceV0 =
                read_json(&golden_path).expect("read committed semantic golden");
            let golden_comparison = compare_semantic_traces(&golden, &first.trace);
            assert!(
                golden_comparison.equal,
                "{} differs from its committed golden: {:?}",
                path.display(),
                golden_comparison.differences
            );
            let replay = replay_semantic_trace(&mut ReferenceSubject::new(), &first.trace);
            assert!(
                replay.equal,
                "captured replay differs: {:?}",
                replay.differences
            );

            let second = run_mock(&loaded);
            let comparison = compare_semantic_traces(&first.trace, &second.trace);
            assert!(
                comparison.equal,
                "exact resimulation differs: {:?}",
                comparison.differences
            );
            assert_eq!(
                semantic_trace_sha256(&first.trace).expect("digest first trace"),
                semantic_trace_sha256(&second.trace).expect("digest second trace")
            );
        }
    }

    #[test]
    fn comparison_ignores_subject_identity_but_rejects_missing_evidence() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../scenarios/v0/nominal_tracking.scenario.json");
        let loaded = load_scenario(&path).expect("load nominal scenario");
        let original = run_mock(&loaded).trace;

        let mut equivalent_subject = original.clone();
        equivalent_subject.subject_id = "equivalent-copper-subject/v0".to_owned();
        assert!(compare_semantic_traces(&original, &equivalent_subject).equal);

        let mut truncated = original.clone();
        truncated.records.pop();
        assert!(validate_trace_evidence(&truncated).is_err());
        assert!(!compare_semantic_traces(&original, &truncated).equal);
    }

    #[test]
    fn benchmark_smoke_keeps_wall_time_out_of_semantic_verdicts() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../scenarios/v0/nominal_tracking.scenario.json");
        let loaded = load_scenario(&path).expect("load nominal scenario");
        let plant = MockSo101Plant::from_scenario(&loaded.scenario).expect("construct mock plant");
        let report = benchmark_with(&loaded, plant, 1, 3).expect("benchmark reference subject");
        assert_eq!(report.iterations, 3);
        assert_eq!(report.semantic_failures, 0);
        assert!(!report.wall_time_is_portable_gate);
        assert_eq!(
            report.control_turn_latency.samples,
            loaded.scenario.ticks.0 * report.iterations
        );
    }
}
