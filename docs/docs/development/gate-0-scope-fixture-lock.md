# Gate 0 scope and fixture lock — frozen acceptance 0.2

**Status:** Accepted (Experimental scope lock; raw execution provenance is local-only)
**Owner:** HefaOS core
**Architecture decision:** Extension at the development-evidence ledger and
the transport-neutral [`Subject` seam](../../../testbench/harness/src/lib.rs).
The fixture and contract remain owned by the test bench; this artifact does not
admit a production runtime, IPC path, or hardware target.

## Acceptance boundary

This artifact freezes the virtual verification workload used by the Gate 0
Copper probe. A change to any pinned input below requires a new scope/fixture
artifact, regenerated evidence, and a new ledger decision; it must not silently
reuse the `0.2` acceptance.

| Included and frozen | Explicitly excluded |
| --- | --- |
| SO-101 v0 mock fixture, pinned MuJoCo model identity, the twelve-scenario semantic corpus, and the nominal static-graph 200 Hz workload | Hardware qualification; a board, image, kernel, firmware, or timing target; all powered actuator work; a physical safety protocol or certification claim; and a ROS bridge or ROS timing/zero-copy claim |

The exclusion is deliberate. This is a Linux-hosted, virtual-time development
fixture. It may expose behavior that a later qualified target must reproduce,
but it does not qualify that target or authorize physical motion.

## Frozen fixture and model identity

| Item | Locked value and source evidence |
| --- | --- |
| Contract/profile | `so101-bench/v0` / `joint-position-loop/v0`, as defined by the [versioned contracts](../../../testbench/contracts/src/lib.rs) |
| Mock fixture | [SO-101 mock plant](../../../testbench/so101/src/mock.rs), reached only through the [bench plant boundary](../../../testbench/so101/src/plant.rs) |
| Model digest | `sha256:5ad49f2b45c083baac9ffe5d4d3213a5da7eac8039095bb2df177a697aae8308` |
| MuJoCo model lock | [Menagerie `robotstudio_so101` lock](../../../testbench/so101/model.lock.toml): commit `da76818e269b82289eba39808e2fb91d679d6994`, `so101.xml` SHA-256 `5ad49f2b45c083baac9ffe5d4d3213a5da7eac8039095bb2df177a697aae8308`, MuJoCo `3.9.0` |
| Model execution files | [Pinned 21-file hash manifest](../../../testbench/so101/execution-files.sha256) |
| Corpus identity lock | [Machine-checked 12-scenario lock](../../../testbench/scenarios/v0/corpus.lock.json) consumed by the Copper evidence CLI |

The mock and physics adapters share the model digest but are different
comparison domains: mock traces can be bit-exact; MuJoCo state is compared with
the declared numeric tolerance. The model lock is a fixture identity, not a
claim that MuJoCo is a hardware-equivalent plant.

## Frozen semantic workload: exactly twelve scenarios

The workload is exactly the twelve files in
[`testbench/scenarios/v0`](../../../testbench/scenarios/v0),
in lexical filename order. Each uses a 5,000,000 ns virtual tick (200 Hz),
the model digest above, `controlAbsoluteTolerance: 1e-9`,
`physicsAbsoluteTolerance: 1e-6`, `requireExactDiscreteTrace: true`, and
`requireBitExactMockTrace: true`.

| File | Semantic name | SHA-256 |
| --- | --- | --- |
| `duplicate_sensor.scenario.json` | `duplicate-sensor-frame` | `dfd789b2a8c32b53f2d1b1f7b296154b8ea242f5769039f03562e79b90d4c624` |
| `estop_latched.scenario.json` | `estop-is-latched` | `577044da61d063d008a2e928b3ee44278f893b42e84675f2ca9dd1af3041da64` |
| `intent_watchdog.scenario.json` | `intent-heartbeat-watchdog-trips` | `69ae79f507e2b1761d36d3777d0d4895e14f5678415abc02cae97a966dd0c3b1` |
| `nominal_tracking.scenario.json` | `nominal-tracking` | `02e5ec2e877d4921743be001917a838415fcdaa3a285e8ae39efcab7176e04ff` |
| `proposal_expired.scenario.json` | `expired-controller-proposal` | `4ea7d98e7c0f601646eac469de1b5fb71c9a40dbb125f02d0caf0023a1fb7155` |
| `proposal_gate_matrix.scenario.json` | `proposal-gate-fail-closed-matrix` | `be24bfe912de3fc1d1b64e83eff28c3b6d7f07c4b267e3445b70152a947ce3e7` |
| `safety_ingress_matrix.scenario.json` | `safety-status-ingress-fail-closed-matrix` | `75d67128f2cb91c1990ef90d4202d3e37c7b69c71016a2ae3bfdd0cb60274cc5` |
| `sensor_ingress_matrix.scenario.json` | `sensor-ingress-fail-closed-matrix` | `8d7b01fbc710d2bf4d58fc022bd31ce685dcaa1dbc3a831fe04460d0ba296988` |
| `setpoint_ingress_matrix.scenario.json` | `setpoint-ingress-fail-closed-matrix` | `42deea68dd1c0850b364a9818b5c64fd5b95d5afe88fc206f1257d1bbfe59579` |
| `stale_safety.scenario.json` | `stale-safety-feedback` | `18b3ba9fb25cebd2f97345710d78876f8a252761a03f5e3f59f7b0870e80b4dc` |
| `subject_task_failure.scenario.json` | `subject-task-failure-latches-safe-state` | `28bc6a4cdb0590985b896d7eabcbea614d665539dd6f571fb5cbbe25d8b1ca4a` |
| `trip_recovery.scenario.json` | `trip-requires-clear-reset-and-new-arm` | `fc94af7d05ebee904e05f1d7bf631fe160251c3f5c4c9dd5ede955a5aaeccbcd` |

The [Copper evidence CLI](../../../testbench/copper-spike/src/main.rs) consumes
the committed corpus lock and rejects an added, removed, renamed, reordered,
semantically renamed, or hash-changed scenario. The semantic trace and
invariant verdict remain the correctness oracle; a retained Copper log is
necessary evidence but cannot replace that oracle.

## Frozen nominal static-graph workload

The timing characterization is one—and only one—scenario:
[`nominal_tracking.scenario.json`](../../../testbench/scenarios/v0/nominal_tracking.scenario.json)
with digest `02e5ec2e877d4921743be001917a838415fcdaa3a285e8ae39efcab7176e04ff`.
It has a static setpoint, no injected faults, 100 virtual turns, and a 5 ms
period: exactly 0.5 s of virtual time at 200 Hz. The direct-Copper subject uses
one static graph per reset—not a per-turn application—and the rate-limited
run must emit the same typed output on every turn as the unpaced semantic run.
The source-injected graph and its retained timing evidence are implemented in
the [Copper subject](../../../testbench/copper-spike/src/lib.rs)
and [evidence command](../../../testbench/copper-spike/src/main.rs).

The required guards are:

- exact discrete decisions and bit-exact mock traces;
- `1e-9` control and `1e-6` physics comparison tolerances, with the scenario's
  independent final-state oracle still enforced;
- a matching output count and exact typed output sequence between paced and
  unpaced Copper runs; and
- retained semantic trace, run manifest, and rate-limited Copper log. Missing
  required evidence fails the run as non-replayable.

The timing report records requested period, observed intervals, missed-list
information, CPU time, peak RSS, binary size, build duration, and log size.
There is **no timing threshold** and no cross-host bit-identical, WCET,
deadline, or hard-real-time claim. The 200 Hz value is a measured workload
target only.

## Reference environment and clean reproduction

The accepted reference is Linux x86_64: Ubuntu 25.10, kernel
`6.17.0-41-generic`, AMD Ryzen 9 3900X, Rust `1.95.0`
(`x86_64-unknown-linux-gnu`), locked Cargo dependencies, and Copper v1.1.1 at
`fc2ebc4fe3583d1f433b75898ad7c9e4dd9e6af2`. The full host, toolchain, command,
stdout, stderr, status, and retained-evidence digest are described in the
[0.1 raw evidence record](evidence/gate-0-copper-v1.1.1-8b79968.md). The
reviewed 1.8 GiB bundle is presently repository-local and unarchived; its
digest is an integrity identifier, not a clone-portable retrieval link. It
therefore supports experimental local inspection only, not accepted portable
raw-evidence provenance. This is a reproduction reference, not a qualified
target profile.

From a clean Linux x86_64 checkout with Rust 1.95.0 installed, reproduce the
frozen workload without overwriting committed evidence:

```bash
export HEFAOS_COPPER_EVIDENCE_DIR="$PWD/target/gate-0-scope-0.2"
rustup run 1.95.0 cargo fmt --all --check
rustup run 1.95.0 cargo clippy --workspace --all-targets --locked -- -D warnings
rustup run 1.95.0 cargo test --workspace --all-targets --locked
rustup run 1.95.0 cargo run --locked -p hefaos-copper-spike -- evidence run-all
rustup run 1.95.0 cargo run --locked -p hefaos-copper-spike -- evidence replay-all
rustup run 1.95.0 cargo run --locked -p hefaos-copper-spike -- evidence timing-nominal
./testbench/tools/with-mujoco.sh
```

`run-all` must retain and accept all twelve scenario identities; `replay-all`
must replay that same retained corpus; and `timing-nominal` must retain the
paced nominal log and matching semantic output. A different host may report
different timing metrics without invalidating the semantic workload, but it
cannot inherit this reference's performance characterization.

The final command is opt-in and networked: the helper reads the repository,
commit, model directory, tree digest, and MuJoCo version from `model.lock.toml`,
then obtains that locked Menagerie revision in an ignored cache, verifies the
model tree and all 21 execution-file SHA-256 values, and runs the MuJoCo test
slice. A missing or mismatched lock value or model asset is a failure. It
verifies lock consistency; it does not make MuJoCo hardware-equivalent or turn
this host into a qualified target.

## Gate disposition

Artifact 0.2 is accepted because it fixes every item in its stated virtual
fixture scope and provides clean reproduction commands. It does **not** accept
the unarchived raw execution bundle and it does **not** close Gate 0.

| Open Gate 0 decision | Accountable owner | Acceptance criterion |
| --- | --- | --- |
| First hardware target/profile and guarded qualification plan | HefaOS core | Name one board, Linux image, kernel, firmware, physical cutoff precondition, and a guarded test plan; accept only target-specific evidence with no powered-work authorization before its physical safeguards are evidenced. |
| ROS comparison protocol (not a bridge implementation) | HefaOS core | Freeze ROS distribution and package pins; graph topology and topic/service directions; message/schema conversion, units and frames; QoS/history/depth/reliability/durability; clock and timestamp mapping; restart and backpressure behavior; and the semantic, timing, copy, and loss metrics. The protocol must run this corpus before any bridge is admitted. |
| Safety-controller target and protocol | HefaOS core | Publish a hazard-boundary decision naming the independent controller/cutoff and its versioned permit/status protocol, including authority, epoch, sequence, TTL, integrity, reset, rollback, and failure behavior. |
| Durable archive for raw Copper 0.1 provenance | HefaOS core | Publish the reviewed bundle or an immutable, clone-portable archive manifest at a stable location that binds its existing aggregate SHA-256 and raw command/status records. |

Hardware qualification, powered work, and a physical safety protocol are not
backfilled by this artifact. A ROS bridge remains excluded and deferred to
Gate 6; the open Gate 0 protocol only fixes the comparison contract it would
later have to meet.
