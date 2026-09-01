//! Regression coverage for the frozen command-level retained-evidence path.

use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn frozen_corpus_command_retains_and_replays_all_scenarios() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after Unix epoch")
        .as_nanos();
    let evidence = std::env::temp_dir().join(format!(
        "hefaos-copper-evidence-cli-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&evidence).expect("create isolated evidence directory");
    let binary = env!("CARGO_BIN_EXE_hefaos-copper-spike");

    let run = Command::new(binary)
        .env("HEFAOS_COPPER_EVIDENCE_DIR", &evidence)
        .args(["evidence", "run-all"])
        .output()
        .expect("run frozen Copper corpus command");
    assert!(
        run.status.success(),
        "run-all failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );

    let replay = Command::new(binary)
        .env("HEFAOS_COPPER_EVIDENCE_DIR", &evidence)
        .args(["evidence", "replay-all"])
        .output()
        .expect("replay retained Copper corpus command");
    assert!(
        replay.status.success(),
        "replay-all failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&replay.stdout),
        String::from_utf8_lossy(&replay.stderr)
    );
    let corpus = fs::read_to_string(evidence.join("corpus-manifest-v1.json"))
        .expect("accepted corpus manifest");
    assert!(corpus.contains("\"status\": \"accepted\""));
    assert_eq!(corpus.matches("run_manifest").count(), 12);

    let timing = Command::new(binary)
        .env("HEFAOS_COPPER_EVIDENCE_DIR", &evidence)
        .args(["evidence", "timing-nominal"])
        .output()
        .expect("run nominal Copper timing command");
    assert!(
        timing.status.success(),
        "timing-nominal failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&timing.stdout),
        String::from_utf8_lossy(&timing.stderr)
    );
    let timing_evidence = fs::read_to_string(evidence.join("nominal-timing-v1.json"))
        .expect("rate-limited nominal timing evidence");
    assert!(
        timing_evidence.contains("\"source\": \"copper_rate_limited_run\""),
        "timing evidence must identify the generated Copper run loop"
    );
    assert!(
        evidence.join("live_nominal.copper").is_file(),
        "timing evidence must retain the CopperList log"
    );
}
