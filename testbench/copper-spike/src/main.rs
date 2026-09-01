use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use hefaos_copper_spike::{CopperSubject, RetainedReplayRequest, replay_retained};
use hefaos_testbench_contracts::ScenarioV0;
use hefaos_testbench_harness::Runner;
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
        let invocation_id = format!(
            "timing-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        );
        let manifest = runner
            .into_subject()
            .finalize(&scenario.name, &entry.sha256, &invocation_id)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(manifest)
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
