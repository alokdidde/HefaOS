# Evidence-Gated Delivery Plan

This plan orders work by uncertainty and safety boundary. A gate is complete only when its exit evidence is committed or linked. Dates and feature counts do not substitute for evidence.

## Current program status

**Active gate:** Gate 0 — in progress. The isolated verification bench is
implemented and experimental; no delivery or target-qualification gate is
complete.

| Decision or evidence | Status | Owner | Acceptance artifact |
|---|---|---|---|
| v2 product and architecture boundary | Draft normative | Unassigned | Concept, specification, limitations review |
| First robot and simulator | Accepted (Experimental virtual fixture) | HefaOS core | [Gate 0 artifact 0.2](gate-0-scope-fixture-lock.md): SO-101 mock, pinned MuJoCo identity, and fixed semantic workload; physical hazard boundary remains open |
| First board, Linux image, kernel, firmware and cutoff | Open | Unassigned | Target profile and guarded test plan |
| Copper capability spike and supported release | Partial (Experimental; raw bundle local-only) | HefaOS core | [Direct Copper acceptance](gate-0-copper-spike.md) and [raw evidence](evidence/gate-0-copper-v1.1.1-8b79968.md) for v1.1.1; durable portable provenance remains open |
| Fixed comparison workload | Partial | HefaOS core | [Gate 0 artifact 0.2](gate-0-scope-fixture-lock.md) accepts the twelve scenarios and static 200 Hz virtual workload; the ROS comparison protocol remains open and no ROS adapter or bridge is admitted |
| Runtime-independent verification bench | Implemented, not qualified | Unassigned | Rust contracts, invariant harness, replay traces, goldens, mock and MuJoCo adapters |
| Restricted `*.hefa.ts` frontend and IR schema | Planned; language selected | Unassigned | Gate 1 fixtures, diagnostics, canonicalization and resource-bound tests |
| Safety-controller target and protocol | Open | HefaOS core | Hazard-boundary decision naming independent controller/cutoff plus a versioned authority, epoch, sequence, TTL, integrity, reset, rollback, and failure protocol |
| ROS 2 mode | Deferred | Unassigned | Gate 6 decision record |
| Fleet persistence / Ignite justification | Deferred | Unassigned | Gate 8 workload evidence |

Gate 0 remains incomplete until every Open item required by that gate has an owner and accepted artifact.

## Gate 0 — Scope and repository truth

Choose one simulated robot, one hardware target, one Linux/kernel profile, and one benchmark workload.

Exit evidence:

- v2 concept, architecture, limitations, and claim policy are canonical;
- every public feature is labeled Concept, Planned, Experimental, Implemented, Measured, or Qualified;
- unsupported hard-real-time, safety, portability, zero-copy, and comparative claims are removed;
- legacy v1 C++ documents and scaffolding are clearly marked superseded;
- direct Copper and ROS 2 comparison workloads are fixed before implementation;
- a thin hand-written Copper spike exercises task lifecycle, messages, recording/replay, intended rate behavior, resources, and any planned iceoryx2 boundary before the IR is frozen;
- a Copper capability/mismatch table identifies which proposed semantics map directly, require a safe lowering, or must be rejected;
- one owner and acceptance criterion exists for every open architecture decision.

## Gate 1 — Hermetic frontend and HefaOS IR

Implement the restricted `*.hefa.ts` AST frontend → canonical IR without
runtime code generation yet. No authored TypeScript is transpiled or executed.

Exit evidence:

- published JSON Schema and IR compatibility policy;
- byte-identical IR for identical complete declared inputs;
- positive/negative discovery fixtures proving that `*.hefa.ts` is admitted
  while ordinary `*.ts`, every `*.tsx`, and JSX are rejected;
- one project-manifest-selected root file, compiler-owned virtual imports, and
  a project lock that binds exact catalogue package versions and artifact
  digests;
- pinned parser, type declarations, frontend, formatter, linter, and upgrade
  policy;
- canonical formatting with golden, cross-host byte-equality, and idempotence
  tests;
- detached source maps and actionable diagnostics with stable rule codes,
  relative spans, deterministic ordering, versioned JSON and SARIF output;
- stable IDs, units, coordinate frames, clock domains, typed ports, finite queues, freshness, and target capabilities;
- deny-by-default AST validation with no transpiled, emitted, imported, or
  executed JavaScript;
- rejection tests for shebangs, triple-slash references, TypeScript suppression
  directives, JSX/compiler pragmas, and source URL/map directives;
- ambient filesystem, environment, wall-clock, randomness, and network access denied;
- Rust build scripts, procedural macros, native builds, and package hooks executed in a network-denied build sandbox with declared inputs and pinned dependencies;
- arbitrary runtime callbacks rejected;
- forbidden-construct, schema migration, malicious-input, and bounded
  source/AST/import/diagnostic/CPU/memory tests;
- golden semantic diagnostics for unknown components/ports, incompatible
  types/units/frames, unsafe cycles, queue/freshness violations, actuator
  bypass, and unsupported backend capabilities;
- machine fixes offered only for already-valid source and restricted to
  transformations proven to preserve its execution-IR digest; invalid source
  receives repair guidance but no automatic fix, and no safety or policy fix is
  automatic;
- whitespace, comments, source paths, and detached authorship/provenance
  changes leave the execution-IR digest unchanged;
- the planned `hefaos fmt --check`,
  `hefaos lint --format sarif --deny-warnings`,
  `hefaos check --format sarif --deny-warnings`, and
  `hefaos build --reproducible` commands work from a clean checkout;
- one `*.hefa.ts` example produces valid IR from a clean checkout.

## Gate 2 — Copper simulation vertical slice

Generate one static sensor → estimator → controller → software motion gate → safety-controller simulator → actuator graph for a mock robot.

Exit evidence:

- generated Copper configuration and inspectable Rust wiring use supported public APIs;
- identical validated IR and complete declared build inputs produce byte-identical generated source;
- the application builds and runs from a clean checkout;
- the same graph runs with a virtual clock and mock HAL;
- every actuator path crosses the software motion gate and a safety-controller simulator with permit/status/epoch feedback;
- stale input, timeout, invalid configuration, task error, and panic behavior are tested;
- replay under the same pinned artifact and execution profile reproduces actuator outputs according to a declared bit-identical or numeric/semantic equality contract, with all required nondeterministic inputs, ticks, drops, faults, and task state captured and restored;
- there is no mutable global runtime state or undeclared graph read;
- HefaOS-generated Copper overhead is measured against equivalent direct Copper.

## Gate 3 — One qualified hardware profile

Run the vertical slice on one named Linux aarch64 board. Powered actuator work is permitted only in a guarded, non-hazardous bench setup with an existing independent physical E-stop/disable and watchdog; otherwise Gate 3 is sensor-only.

Exit evidence:

- locked Rust, Copper, system image, firmware, and dependency versions;
- target manifest for CPU, kernel, IRQ, clock, memory, device, and permissions;
- the guarded bench setup documents and tests its pre-existing independent physical E-stop/cutoff and watchdog before any powered actuator test;
- recorded firmware/BIOS, SMT, CPU-frequency, C-state, clocksource, and relevant firmware-SMI conditions;
- allocation guard for the critical phase;
- no blocking filesystem, network, database, UI, AI, or ordinary logging call in that phase;
- sustained-load timing with p50/p95/p99/p99.9/max, jitter, and missed deadlines;
- CPU, memory, thermal, I/O, recorder, AI, and network load conditions documented;
- raw data and reproduction commands published;
- results described as qualified only for that target profile.

## Gate 4 — Safety and secure deployment boundary

Integrate the target safety controller and secure deployment boundary before leaving the guarded bench profile. The simulator and minimum physical cutoff required for Gate 3 already exist by this point.

Exit evidence:

- target hazard analysis and robot-specific safe/degraded states;
- packed, versioned safety protocol with epoch, sequence, TTL, integrity, authority, reset, and rollback behavior;
- every actuator path passes through the software motion gate and independent controller;
- byte-reproducible content bundle and digest, detached signature/provenance verification, SBOM, atomic activation, and last-known-good rollback;
- device/operator identity and least privilege;
- fault injection for crash, hang, stale/duplicate/corrupt command, device loss, reboot during update, invalid config, E-stop, and controlled reset;
- no claim of certification beyond the available evidence.

## Gate 5 — One isolated AI path

Add one non-authoritative perception model in a separate lower-priority process. An in-process microbenchmark MAY be used only as a performance baseline and MUST NOT share the deterministic control process or be described as fault-contained.

Exit evidence:

- pinned model, preprocessing, postprocessing, runtime, and target hashes;
- warm-up, CPU/accelerator, memory, queue, and concurrency budgets;
- timestamps, source sequence, maximum input/output age, confidence, validity, timeout, and fallback;
- within the declared and tested user-space fault and resource-isolation envelope, controller integrity is preserved during AI crash, hang, overload, GPU reset/stall, late output, and malformed output; host-wide failure behavior is verified at the independent safety boundary;
- process boundary uses a tested bounded schema and explicit copy/staging behavior;
- iceoryx2 pool exhaustion, borrower death, restart, and incompatible schema tests;
- recording captures or substitutes AI outputs for replay;
- AI has no direct actuator authority.

## Gate 6 — Minimum ROS 2 adoption bridge

Bridge only the topics needed by one pilot; add services/actions later.

Exit evidence:

- pinned ROS distribution and bridge implementation;
- explicit schema, QoS, clock, frame, unit, queue, copy, and restart semantics;
- within the declared and tested user-space fault and resource-isolation envelope, bridge failure does not block or corrupt native control;
- interoperability tests cover stale data, queue overflow, conversion errors, and clock mismatch;
- no end-to-end zero-copy or deterministic timing claim across the bridge.

## Gate 7 — Fleet edge agent

Implement database-neutral desired/observed reconciliation before choosing a database.

Exit evidence:

- signed desired-state cache and bounded durable outbox;
- robot/deployment epoch, revision, command ID, expiry, idempotency, acknowledgement, and fencing where required;
- stale, expired, duplicated, reordered, unauthorized, and conflicting commands are rejected;
- offline operation through prolonged network loss;
- idempotent reconciliation after reconnect and agent restart;
- telemetry batching/downsampling and backpressure isolation;
- fleet services cannot directly command actuators.

## Gate 8 — Persistence decision

Select Apache Ignite only if measured scale, consistency, query, or availability requirements justify it.

Exit evidence for an Ignite adapter:

- workload and scale comparison against simpler storage options;
- deployment and operational ownership plan;
- supported-client gateway isolated from the robot critical path;
- majority-loss, partition, failover, transaction abort/retry, and recovery tests;
- idempotent data-stream ingestion;
- raw high-rate telemetry stored outside Ignite;
- within the declared and tested service/user-space isolation envelope, complete Ignite loss does not block or corrupt local control; the robot follows its offline/degradation policy, including safe stop where required, and no safety function depends on Ignite.

## Gate 9 — Broader product surface

Additional boards, simulators, AI runtimes, behavior libraries, fleet coordination, visual tools, and ROS interfaces enter individually with their own conformance profile.

Read-only graph and timeline visualization SHOULD precede bidirectional visual editing. Hardware hot reload remains prohibited; signed deployments and rollback remain the production path.

## Gate 10 — Additional execution backend

Add another backend only for a concrete requirement Copper cannot meet.

Exit evidence:

- a user workload and missing Copper semantic;
- an attempted upstream extension or documented reason it is unsuitable;
- capability mapping and fail-closed diagnostics;
- full backend conformance suite;
- lifecycle, security, replay, and safety burden funded and owned;
- no regression to the proven Copper path.

A native HefaOS scheduler may never be necessary.

## Initial benchmark matrix

| Question | Comparison | Required output |
|---|---|---|
| Does HefaOS generation add runtime cost? | Direct Copper vs HefaOS-generated Copper | Latency, jitter, CPU, memory, binary/build cost |
| Does HefaOS improve agent-first authoring? | Direct Copper project vs restricted `*.hefa.ts` workflow | First-pass success, repair iterations, stable diagnostics consumed, invalid graphs caught before build, generated diff size, and occasional human-review time |
| Does process isolation contain declared failures? | In-process model vs bounded isolated model | Failure containment, copy cost, queue/pool behavior, control timing |
| Does the robot handle fleet loss? | Connected vs partitioned/failed fleet services | Controller integrity, declared degradation response, bounded storage, reconciliation correctness |
| Is ROS migration viable? | Equivalent ROS 2 and bridged workflow | Semantic coverage, conversion cost, QoS/clock behavior, operational steps |

Benchmarks MUST be designed before optimization and MUST retain raw evidence.

## Deferred work

The following are intentionally deferred until prior gates pass:

- central Arrow/ECS runtime state;
- custom executor or scheduler;
- generic hard-real-time claims;
- multiple physics engines;
- broad board and driver catalogs;
- safety-critical AI or LLM actuation;
- dynamic hardware graph mutation;
- production hot model swapping;
- bidirectional visual-source editing;
- Ignite as a mandatory dependency;
- transparent backend portability.
