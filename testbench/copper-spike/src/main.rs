use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use hefaos_copper_spike::{
    CopperSubject, RetainedReplayRequest, replay_retained, run_rate_limited_source_injected,
};
use hefaos_testbench_contracts::{ScenarioV0, SemanticTraceV0};
use hefaos_testbench_harness::{Runner, compare_semantic_traces};
use hefaos_testbench_so101::MockSo101Plant;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next().as_deref(), args.next()) {
        (Some("evidence"), Some("run-all"), None) => run_all(),
        (Some("evidence"), Some("replay-all"), None) => replay_all(),
        (Some("evidence"), Some("timing-nominal"), None) => run_nominal_timing(),
        _ => bail!("usage: hefaos-copper-spike evidence <run-all|replay-all|timing-nominal>"),
    }
}

fn replay_all() -> Result<()> {
    let root = evidence_root();
    let corpus: CorpusManifest =
        serde_json::from_slice(&fs::read(root.join("corpus-manifest-v1.json"))?)?;
    if corpus.schema_version != 1 || corpus.status != "accepted" || corpus.scenarios.len() != 12 {
        bail!("invalid Copper corpus manifest shape");
    }
    let mut actual = BTreeSet::new();
    let mut run_manifests = BTreeSet::new();
    for entry in &corpus.scenarios {
        if !actual.insert((entry.name.clone(), entry.sha256.clone())) {
            bail!("duplicate Copper corpus manifest scenario {}", entry.name);
        }
        if !run_manifests.insert(entry.run_manifest.clone()) {
            bail!(
                "duplicate Copper corpus run manifest {}",
                entry.run_manifest.display()
            );
        }
    }
    let expected = frozen_scenarios()?;
    if actual
        != expected
            .iter()
            .map(|entry| (entry.name.clone(), entry.sha256.clone()))
            .collect()
    {
        bail!("Copper corpus manifest does not exactly match frozen scenario identities/digests");
    }
    let requests = corpus
        .scenarios
        .iter()
        .map(|entry| RetainedReplayRequest {
            manifest_path: root.join(&entry.run_manifest),
            scenario_name: entry.name.clone(),
            scenario_sha256: entry.sha256.clone(),
            invocation_id: corpus.invocation_id.clone(),
        })
        .collect::<Vec<_>>();
    let replayed = match replay_retained(&requests) {
        Ok(replayed) => replayed,
        Err(error) => {
            let failed = CorpusManifest {
                status: "non-replayable".to_owned(),
                ..corpus
            };
            write_corpus(&root, &failed)?;
            return Err(anyhow::anyhow!(error.to_string()));
        }
    };
    println!("replayed {replayed} retained Copper evidence runs");
    Ok(())
}

fn run_all() -> Result<()> {
    let root = evidence_root();
    let scenarios = frozen_scenarios()?;
    fs::create_dir_all(&root)?;
    let invocation_id = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    );
    write_corpus(
        &root,
        &CorpusManifest {
            schema_version: 1,
            invocation_id: invocation_id.clone(),
            status: "running".to_owned(),
            scenarios: Vec::new(),
        },
    )?;
    let mut completed = Vec::with_capacity(scenarios.len());
    let result = (|| -> Result<()> {
        for entry in scenarios {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../scenarios/v0")
                .join(&entry.file);
            let raw = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
            let scenario: ScenarioV0 = serde_json::from_slice(&raw)
                .with_context(|| format!("parse {}", path.display()))?;
            let plant = MockSo101Plant::from_scenario(&scenario)?;
            let mut runner = Runner::new(CopperSubject::new(), plant);
            let outcome = runner.run(&scenario, &entry.sha256)?;
            if !outcome.verdict.passed {
                bail!(
                    "Copper scenario {} failed: {}",
                    scenario.name,
                    outcome.verdict.failures.join("; ")
                );
            }
            verify_committed_golden(&entry.file, &outcome.trace)?;
            let manifest = runner
                .into_subject()
                .finalize(&scenario.name, &entry.sha256, &invocation_id)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
            completed.push(CorpusScenario {
                name: scenario.name.clone(),
                sha256: entry.sha256,
                run_manifest: manifest
                    .strip_prefix(&root)
                    .with_context(|| {
                        format!("manifest outside evidence root {}", manifest.display())
                    })?
                    .to_path_buf(),
            });
            println!("{}", scenario.name);
        }
        write_corpus(
            &root,
            &CorpusManifest {
                schema_version: 1,
                invocation_id: invocation_id.clone(),
                status: "accepted".to_owned(),
                scenarios: completed.clone(),
            },
        )
    })();
    if let Err(error) = result {
        write_corpus(
            &root,
            &CorpusManifest {
                schema_version: 1,
                invocation_id,
                status: "non-replayable".to_owned(),
                scenarios: completed,
            },
        )?;
        return Err(error);
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn run_nominal_timing() -> Result<()> {
    let root = evidence_root();
    fs::create_dir_all(&root)?;
    let entry = frozen_scenarios()?
        .into_iter()
        .find(|entry| entry.name == "nominal-tracking")
        .context("frozen corpus has no nominal-tracking scenario")?;
    let initial_status = NominalTimingStatus {
        schema_version: 1,
        status: "running".to_owned(),
        scenario_name: entry.name.clone(),
        scenario_sha256: entry.sha256.clone(),
        run_manifest: None,
    };
    write_nominal_timing_status(&root, &initial_status)?;
    let result = (|| -> Result<PathBuf> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../scenarios/v0")
            .join(&entry.file);
        let raw = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let scenario: ScenarioV0 =
            serde_json::from_slice(&raw).with_context(|| format!("parse {}", path.display()))?;
        let plant = MockSo101Plant::from_scenario(&scenario)?;
        let mut runner = Runner::new(CopperSubject::new(), plant);
        let outcome = runner.run(&scenario, &entry.sha256)?;
        if !outcome.verdict.passed {
            bail!(
                "Copper nominal timing scenario failed: {}",
                outcome.verdict.failures.join("; ")
            );
        }
        verify_committed_golden(&entry.file, &outcome.trace)?;
        let invocation_id = format!(
            "timing-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        );
        let manifest = runner
            .into_subject()
            .finalize(&scenario.name, &entry.sha256, &invocation_id)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let inputs = outcome
            .trace
            .records
            .iter()
            .map(|record| record.subject_input.clone())
            .collect::<Vec<_>>();
        let paced_log = root.join("live_nominal.copper");
        let paced =
            run_rate_limited_source_injected(&scenario.subject_config(), &inputs, &paced_log)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        if paced.outputs.len() != outcome.trace.records.len() {
            bail!(
                "Copper rate-limited timing output count {} differs from harness trace {}",
                paced.outputs.len(),
                outcome.trace.records.len()
            );
        }
        for (index, (recorded, expected)) in
            paced.outputs.iter().zip(&outcome.trace.records).enumerate()
        {
            if recorded.output != expected.subject_output {
                bail!(
                    "Copper rate-limited timing output at turn {index} differs from the harness trace"
                );
            }
        }
        let timing_manifest = NominalTimingEvidence {
            schema_version: 1,
            scenario_name: scenario.name.clone(),
            scenario_sha256: entry.sha256.clone(),
            unpaced_run_manifest: manifest
                .strip_prefix(&root)
                .with_context(|| format!("manifest outside evidence root {}", manifest.display()))?
                .to_path_buf(),
            rate_limited_log: paced_log
                .strip_prefix(&root)
                .with_context(|| {
                    format!("timing log outside evidence root {}", paced_log.display())
                })?
                .to_path_buf(),
            timing: paced.timing,
        };
        let timing_manifest_path = root.join("nominal-timing-v1.json");
        write_json(&timing_manifest_path, &timing_manifest)?;
        Ok(timing_manifest_path)
    })();
    match result {
        Ok(manifest) => {
            let status = NominalTimingStatus {
                schema_version: 1,
                status: "accepted".to_owned(),
                scenario_name: entry.name,
                scenario_sha256: entry.sha256,
                run_manifest: Some(
                    manifest
                        .strip_prefix(&root)
                        .with_context(|| {
                            format!("manifest outside evidence root {}", manifest.display())
                        })?
                        .to_path_buf(),
                ),
            };
            write_nominal_timing_status(&root, &status)?;
            println!("{}", manifest.display());
            Ok(())
        }
        Err(error) => {
            write_nominal_timing_status(
                &root,
                &NominalTimingStatus {
                    schema_version: 1,
                    status: "non-replayable".to_owned(),
                    scenario_name: entry.name,
                    scenario_sha256: entry.sha256,
                    run_manifest: None,
                },
            )?;
            Err(error)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CorpusManifest {
    schema_version: u16,
    invocation_id: String,
    status: String,
    scenarios: Vec<CorpusScenario>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CorpusScenario {
    name: String,
    sha256: String,
    run_manifest: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct NominalTimingStatus {
    schema_version: u16,
    status: String,
    scenario_name: String,
    scenario_sha256: String,
    run_manifest: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct NominalTimingEvidence {
    schema_version: u16,
    scenario_name: String,
    scenario_sha256: String,
    unpaced_run_manifest: PathBuf,
    rate_limited_log: PathBuf,
    timing: hefaos_copper_spike::TimingObservation,
}
#[derive(Debug)]
struct FrozenScenario {
    file: String,
    name: String,
    sha256: String,
}

fn evidence_root() -> PathBuf {
    std::env::var_os("HEFAOS_COPPER_EVIDENCE_DIR")
        .map_or_else(|| "target/copper-spike".into(), PathBuf::from)
}

fn write_corpus(root: &Path, manifest: &CorpusManifest) -> Result<()> {
    let temporary = root.join("corpus-manifest-v1.json.tmp");
    let final_path = root.join("corpus-manifest-v1.json");
    fs::write(&temporary, serde_json::to_vec_pretty(manifest)?)?;
    fs::rename(&temporary, &final_path)?;
    Ok(())
}

fn write_nominal_timing_status(root: &Path, status: &NominalTimingStatus) -> Result<()> {
    let temporary = root.join("nominal-timing-status-v1.json.tmp");
    let final_path = root.join("nominal-timing-status-v1.json");
    fs::write(&temporary, serde_json::to_vec_pretty(status)?)?;
    fs::rename(&temporary, &final_path)?;
    Ok(())
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(value)?)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

fn verify_committed_golden(scenario_file: &str, trace: &SemanticTraceV0) -> Result<()> {
    let golden_name = scenario_file.replace(".scenario.json", ".semantic-trace.json");
    if golden_name == scenario_file {
        bail!("invalid frozen scenario filename {scenario_file}");
    }
    let golden_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../goldens/v0")
        .join(golden_name);
    let golden: SemanticTraceV0 = serde_json::from_slice(
        &fs::read(&golden_path).with_context(|| format!("read {}", golden_path.display()))?,
    )
    .with_context(|| format!("parse {}", golden_path.display()))?;
    let comparison = compare_semantic_traces(&golden, trace);
    if !comparison.equal {
        bail!(
            "Copper semantic trace differs from committed golden {}: {:?}",
            golden_path.display(),
            comparison.differences
        );
    }
    Ok(())
}

fn frozen_scenarios() -> Result<Vec<FrozenScenario>> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scenarios/v0");
    let mut scenarios: Vec<_> = fs::read_dir(&directory)
        .with_context(|| format!("read {}", directory.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<_, _>>()?;
    scenarios.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "json")
    });
    scenarios.sort();
    if scenarios.len() != 12 {
        bail!("expected 12 frozen scenarios, found {}", scenarios.len());
    }
    let mut frozen = Vec::with_capacity(scenarios.len());
    for path in scenarios {
        let raw = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        let scenario: ScenarioV0 =
            serde_json::from_slice(&raw).with_context(|| format!("parse {}", path.display()))?;
        frozen.push(FrozenScenario {
            file: path
                .file_name()
                .and_then(|name| name.to_str())
                .context("non-utf8 scenario filename")?
                .to_owned(),
            name: scenario.name,
            sha256: format!("{:x}", Sha256::digest(raw)),
        });
    }
    Ok(frozen)
}
