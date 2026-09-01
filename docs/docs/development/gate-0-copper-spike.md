# Gate 0 Copper spike — frozen acceptance

**Status:** Planned acceptance artifact 0.1
**Owner:** HefaOS core
**Decision:** Extend the transport-neutral `testbench` `Subject` boundary with a
direct, hand-written Copper application. This is an experimental probe, not a
production backend, compiler, replacement scheduler, or a change to the
versioned bench contracts.

## Frozen inputs

| Item | Frozen value |
| --- | --- |
| Copper source | `https://github.com/copper-project/copper-rs` tag `v1.1.1`, commit `fc2ebc4fe3583d1f433b75898ad7c9e4dd9e6af2` |
| Rust toolchain | `1.95.0` with `rustfmt` and `clippy`; migration is atomic with the Copper pin |
| Robot and simulator | existing SO-101 v0 mock plant; pinned MuJoCo remains opt-in and is not a Copper acceptance substitute |
| Correctness workload | all twelve committed `testbench/scenarios/v0/*.scenario.json` scenarios and their hand-written expectations and semantic goldens |
| Timing workload | same nominal SO-101 scenario, static Copper graph, `rate_target_hz: 200`; host timing is collected only, not used as a portable pass/fail target |
| Execution environment | clean Linux x86_64 checkout, locked Cargo dependencies, Rust `1.95.0`; the evidence records `rustc -Vv`, OS, CPU, and Copper revision |
| Equality contract | Copper source-injected simulation and Copper replay must reproduce the declared typed output sequence exactly. The bench's existing captured-input semantic comparison remains its separate, transport-neutral oracle. No cross-host bit-identical or real-time claim is made. |

## Acceptance criteria

The artifact passes only when all of the following are true:

1. Cargo pins only Copper public crates at the frozen commit, with the resolved
   revision recorded in `Cargo.lock`; the workspace and CI use Rust `1.95.0`.
2. A direct, hand-written Copper graph exercises task construction, start,
   process, stop, typed messages, `Freezable` task state, named resources,
   source simulation, bounded recording, and replay using public APIs.
3. The Copper subject implements the existing `Subject` seam without changing
   `testbench/contracts` serialization or allowing the subject to reach a plant
   directly. Reset, injected task error, invalid input, and panic paths fail
   closed through the existing harness semantics.
4. Deterministic positive, negative, fault, replay, regression, and clean
   checkout reproducibility tests cover the frozen corpus. A safe-but-motionless
   subject remains rejected by the existing independent oracle.
5. A raw upstream Copper example run and raw HefaOS spike runs are retained
   under an evidence directory with commands, stdout/stderr, exit status,
   revision, environment, generated configuration, log digests, and verdicts.
6. Formatting, warning-free clippy, and all workspace tests pass. The final
   clean-checkout command is:

   ```bash
   rustup run 1.95.0 cargo fmt --all --check
   rustup run 1.95.0 cargo clippy --workspace --all-targets --locked -- -D warnings
   rustup run 1.95.0 cargo test --workspace --all-targets --locked
   ```

7. A separate Copper `cu_iceoryx2_bridge` v1.1.1 loopback characterization is
   recorded. Its current bincode/`Vec` copy and missing declared
   queue/schema/epoch/pool policies mean it is **not admitted** on the SO-101
   control edge; this spike must not introduce an alternate IPC bus.

## Metrics and failure policy

The timing run records iteration count, requested 5 ms period, observed loop
interval distribution, missed Copper-list count, process CPU time, peak RSS,
binary size, build duration, and recording-log size. There is intentionally no
threshold until the direct-Copper baseline and a pinned benchmark machine are
accepted. A run with missing required trace or recording evidence is failed and
marked non-replayable; it is not silently retried or summarized as a pass.

## Reproduction commands

The final evidence must contain the exact commands, including the upstream
reference command and its output. The expected shape is:

```bash
git clone --branch v1.1.1 https://github.com/copper-project/copper-rs.git /tmp/copper-rs
cd /tmp/copper-rs && rustup run 1.95.0 cargo run -p cu-run-in-sim
rustup run 1.95.0 cargo run --locked -p hefaos-copper-spike -- evidence run-all
rustup run 1.95.0 cargo run --locked -p hefaos-copper-spike -- evidence replay-all
```

## Capability disposition

| Proposed semantic | Gate 0 disposition |
| --- | --- |
| Static graph, task lifecycle, typed local messages, resources, source simulation, recording/replay | Exercise directly through Copper v1.1.1 public APIs |
| 200 Hz whole-loop target | Measure only; it does not establish per-task deadlines, WCET, or a hard-real-time claim |
| Copper replay | Exercise and verify for this source-injected graph; do not treat it as complete HefaOS admitted replay evidence yet |
| iceoryx2 bridge | Characterize separately; reject from control admission pending bounded schema, queue, epoch, pool, failure, and replay qualification |
| Non-harmonic rates, arbitrary callbacks, global state, a custom executor | Reject/defer; no safe lowering is admitted by this spike |
