# HefaOS verification test bench

This directory is the first executable HefaOS v2 slice. It is a verification
system, not a robot runtime. The bench owns virtual time, scenarios, fault
injection, the independent safety-controller simulator, plant adapters,
semantic traces, comparison, and verdicts.

The graph under test is deliberately opaque. Today that graph is a small
sequential reference subject used to prove the bench. Later, the same captured
inputs and invariant checks will be used for a hand-written Copper graph and
the equivalent HefaOS-generated Copper graph. On reset, a subject receives
only `SubjectConfigV0`; future faults, expected answers, equality policy, and
plant configuration remain private to the bench.

## Boundary

```text
scenario + virtual clock
          |
          v
SO-101 plant -- authoritative observation --> safety-controller simulator
      |                                              ^           |
      +-- observation -- deterministic faults --> subject        |
                                                   |             |
                                                   +-- intent ---+
                                                                 |
                                                    authorized actuation
                                                                 |
                                                                 v
                                                        mock or MuJoCo plant

Every turn also emits:
effective subject input + scheduled events + subject output
          + authoritative safety observation + safety state
                       + disposition + plant state
                                      |
                                      v
                    complete semantic trace and verdict
```

The subject cannot write the plant directly. The bench records controller
output, gate disposition, safety state, and applied actuation as distinct
values. Safety starts disarmed, source and permit epochs are fenced, intent
freshness is half-open, and an independent per-turn slew guard checks intent
against the authoritative pre-actuation plant observation. A heartbeat
watchdog trips an armed controller that stops producing valid intent. A trip
is latched; disarm and clear-faults cannot bypass reset and a later re-arm.

## What v0 verifies

- fixed SO-101 joint order and radians plus a normalized gripper channel;
- integer virtual timestamps, epochs, strictly ordered sequences, and
  half-open validity intervals;
- finite values, model limits, freshness, permit epochs, subject rate limits,
  and an independent safety slew limit;
- deterministic faults for missing, stale, invalid, duplicated, reordered,
  future-dated, incompatible, and task-failure inputs, including exact
  half-open deadline boundaries on every ingress port;
- corpus coverage for every declared v0 fault branch and all five proposal
  fault modes, guarded by an exhaustive regression test;
- zero newly authorized motion while disarmed or tripped;
- replay from captured subject inputs and recorded safety events;
- deterministic mock resimulation and committed semantic regression goldens;
- a hand-authored nominal final-state oracle that rejects safe-but-motionless
  subjects on both mock and pinned physics plants;
- invariant rejection of incomplete, non-finite, internally inconsistent, or
  non-replayable evidence;
- headless integration with the pinned MuJoCo Menagerie SO-101 model;
- machine-readable benchmark reports without portable wall-time assertions.

This phase contains no Copper adapter, compiler, HefaOS production runtime,
hardware driver, camera, VLA model, network service, or safety certification.

## Default verification

The default workspace has no native simulator or network requirement:

```bash
cargo fmt --all --check
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

The CLI runs the 12 versioned scenarios, writes semantic traces, replays their
captured inputs, compares traces across subject implementations, and produces
benchmark reports:

```bash
cargo run --locked -p hefaos-testbench-cli -- list
cargo run --locked -p hefaos-testbench-cli -- \
  run testbench/scenarios/v0/nominal_tracking.scenario.json
cargo run --locked -p hefaos-testbench-cli -- \
  replay testbench/scenarios/v0/nominal_tracking.scenario.json
cargo run --locked -p hefaos-testbench-cli -- \
  benchmark testbench/scenarios/v0/nominal_tracking.scenario.json \
  --warmup 10 --iterations 100
```

## Pinned MuJoCo verification

MuJoCo is opt-in. The helper checks out exactly the Menagerie revision recorded
in [`so101/model.lock.toml`](so101/model.lock.toml), verifies all 21 XML and
mesh execution-file hashes in [`so101/execution-files.sha256`](so101/execution-files.sha256),
and lets `mujoco-rs` download and verify MuJoCo 3.9.0 in the ignored cache.

```bash
./testbench/tools/with-mujoco.sh
```

To run another command in the same prepared environment:

```bash
./testbench/tools/with-mujoco.sh \
  cargo run --locked -p hefaos-testbench-cli --features mujoco -- --help
```

If MuJoCo is explicitly requested, missing or mismatched assets are a failure,
not a skipped test. Physics trajectories use declared numeric tolerances;
cross-platform bit-identical MuJoCo behavior is not claimed.

## Evidence policy

Correctness uses virtual time and exact discrete decisions. Each trace records
the safety oracle's pre-actuation observation and requires it to equal the
initial or preceding authoritative plant state, so a replay cannot invent a
more permissive slew baseline. Scenario and trace validation also require the
independent safety policy to be no looser than the subject's gate policy. Mock
traces are bit-exact except for subject
identity, which is provenance rather than graph behavior; MuJoCo state uses
declared tolerances. Host latency is reported separately and is not a shared-CI
pass/fail threshold until a pinned benchmark machine and direct-Copper baseline
exist. Any missing required trace evidence makes a run non-replayable and fails
verification.

The committed files under [`goldens/v0`](goldens/v0) are regression baselines,
not the safety oracle. Hand-authored scenario expectations and independent
per-record safety invariants remain authoritative. This phase still has no
Copper adapter or subprocess transport; the public Rust `Subject` boundary is
the intentional seam for that next iteration.
