# Architecture Review and Limitations

**Review date:** 2026-09-01
**Applies to:** HefaOS v1 specifications and the proposed v2 Rust direction
**Disposition:** v1 is superseded; unresolved v2 risks remain explicit

## Executive finding

The v1 documents should not be translated line-for-line from C++ into Rust. Rust improves memory safety and toolchain coherence, but it does not resolve conflicting execution models, undefined timing semantics, unsafe authority boundaries, or excessive product scope.

The repository is inexpensive to redirect because its runtime and SDK are mostly scaffolding. That is also why no current performance, determinism, zero-copy, safety, or end-to-end feature claim should be treated as implemented.

## Problems found and v2 resolutions

| Problem in v1 | Why it fails | v2 resolution | Residual limitation |
|---|---|---|---|
| C++ is embedded through the runtime, SDK generator, build, examples, and CI | A search-and-replace port would preserve the wrong architecture | Rust task crates and Cargo; restricted `*.hefa.ts` → HefaOS IR → backend | Existing C++ scaffolding remains until deliberately removed |
| “Explicit dependencies” coexist with global `StateStore` reads, reactive signals, effects, and callbacks | Hidden reads make graph analysis and deterministic replay unreliable | Typed graph ports are the only critical-path data dependencies | Non-real-time tools still need carefully labeled snapshots |
| Arrow/ECS/MVCC is the universal state model | Analytical layout, COW, locks, raw pointers, and heap queries conflict with bounded control execution | Local preallocated messages for control; Arrow/Parquet only in asynchronous export | Conversion and storage can lag or lose nonessential telemetry |
| iceoryx, FlatBuffers, Arrow, and local access are all called “zero copy” | Translation, serialization, DMA, GPU upload, bridges, and export can copy | Document every copy boundary; call iceoryx2 “zero-copy-capable local transport” | Cache effects and ownership recovery still add latency variance |
| Work stealing, automatic parallelization, priority execution, and task preemption are promised together | These mechanisms conflict, and arbitrary Rust/C++ tasks cannot be safely preempted | Copper owns initial execution; HefaOS validates declared contracts and measures outcomes | Copper may not support every desired multi-rate/deadline semantic |
| Hard-real-time and 10–100× ROS 2 claims lack evidence | PREEMPT_RT, Rust, and p99 measurements do not establish WCET or certification | Publish a measured latency envelope and miss distribution for each qualified target profile | An observed maximum is not a proven worst-case bound and does not generalize universally |
| AI tasks receive hard deadlines and direct authority | GPU inference is commonly variable and not application-preemptible | AI is isolated, freshness-bounded, recorded, and non-authoritative | Some future learned controllers may need a separate safety case |
| Safety is expressed as watchdog examples | Safe stop, brake behavior, reset authority, replay protection, and hazard analysis are target-specific | Independent safety controller, software motion gate, authenticated reset, explicit safe states | HefaOS is not safety certified merely by implementing these mechanisms |
| The central process, shared memory, and GPU are described as fault isolated | Processes still share kernel, memory pools, drivers, buses, and resources | Explicit execution islands, budgets, supervision, epochs, queue recovery, and fault injection | Process isolation reduces blast radius but is not physical independence |
| Arbitrary TS callbacks are expected to become native code | Closures, async effects, filesystem access, wall time, and JavaScript numeric semantics are not a deterministic DSL | `*.hefa.ts` is parsed through a deny-by-default declarative AST only; Rust symbols implement runtime behavior | TypeScript familiarity can mislead users and agents because editors and generic TS tools accept constructs HefaOS rejects |
| “Backend-neutral” is assumed to mean portable behavior | Backends disagree on releases, deadlines, queues, clocks, replay, cancellation, and process boundaries | Required-capability declarations plus fail-closed lowering and conformance tests | The IR may still evolve around the first backend unless actively governed |
| HefaOS plans to compete with Copper while also using it | Double scheduling and misleading performance comparisons would result | Copper is the first engine; compare generated Copper with direct Copper for overhead and workflow value | Dependency/version compatibility becomes a product responsibility |
| Ignite is considered a replacement runtime state store | A transactional distributed database has variable network, lock, replica, and retry behavior | Optional database-neutral fleet state adapter behind a gateway | Ignite adds operational and JVM/client complexity and may be unnecessary at small scale |
| Fleet “shared state” has no protocol | Storage does not define authority, fencing, expiry, idempotency, or reconciliation | Separate desired/observed state with robot epoch, revision, command ID, expiry, signature, acknowledgement, and optional lease | Cross-robot physical coordination remains an application problem |
| ROS 2 migration is described as field conversion and `rmw_iceoryx` replacement | QoS, clocks, frames, services/actions, lifecycle, and failure behavior do not map automatically | An optional non-real-time sidecar bridge with explicit per-interface contracts | Rust ROS bindings and bridge choices require validation per supported ROS release |
| Replay is treated as a file format | Reproducibility also requires artifact hash, clock, inputs, drops, faults, model outputs, and task state | Separate playback, deterministic replay, and counterfactual resimulation contracts | Physical systems and nondeterministic external services cannot be recreated unless captured |
| Same binary is promised across ARM boards | CPU features, drivers, ABI, kernel, peripherals, and board manifests differ | Same source and IR can be rebuilt for qualified target profiles | Each target still requires validation and may produce distinct artifacts |
| Security is absent while remote deployment and fleet access are planned | A compromised update or command channel can become physical harm | Device identity, mTLS, signed bundles, rollback protection, RBAC, audit, and secrets isolation | Secure boot and key storage depend on target hardware |
| The MVP includes a scheduler, compiler, HAL, AI serving, fleet, simulators, IDE, ROS, and many boards | This is several products and prevents an evidence-backed vertical slice | One `*.hefa.ts`→IR→Copper simulated robot first; all other capabilities are gated | Narrow scope may initially look less impressive than the original vision |

## Evidence from the existing repository

The most important contradictions remain visible in the superseded documents and scaffold:

- [Legacy OS specification](https://github.com/alokdidde/HefaOS/blob/master/hefaos-os-specification.md) rejects hidden global dependencies, then schedules tasks through a global state store.
- The same document combines Arrow state, MVCC/copy-on-write, locks, raw access, and broad real-time claims without a lifetime or worst-case timing model.
- [Legacy SDK specification](https://github.com/alokdidde/HefaOS/blob/master/hefaos-sdk-specification.md) permits validators, closures, async behavior actions, effects, and direct I/O while also saying TypeScript does not run on the robot.
- [Legacy development infrastructure](https://github.com/alokdidde/HefaOS/blob/master/hefaos-dev-infrastructure.md) assumes CMake, legacy iceoryx/RouDi, and C++ tests throughout.
- `runtime/core/src/executor.cpp` and `task_graph.cpp` are TODO shells; HAL and AI sources are also placeholders.
- `StateStore::create_entity()` returns a fixed identifier and `is_alive()` returns false.
- Existing tests largely assert unconditional success.
- The compiler emits placeholder C++ strings and exports modules that do not yet exist.
- The changelog and README previously described planned components in present tense.

These facts are not defects to patch individually. They demonstrate that v2 should begin from an explicit semantic contract and a proving vertical slice.

## Limitations of the revised strategy

### Copper dependency

Using Copper removes a large runtime burden, but introduces compatibility and product-positioning risk:

- HefaOS must pin a supported Copper release and use public APIs only.
- Compatibility CI must test the pinned release and the newest supported release.
- Direct Copper and HefaOS-generated Copper must be compared to expose generator overhead.
- HefaOS cannot claim runtime superiority when the generated application uses Copper.
- Copper facilities for bridges, logging, replay, simulation, resources, and lifecycle should be reused before HefaOS creates parallel implementations.
- If the backend requires private internals or constant patches, the strategy should be reconsidered.

### Agent-first TypeScript frontend

Choosing a familiar TypeScript suffix improves parser, editor, and agent
compatibility; it does not make unrestricted TypeScript deterministic or safe.
HefaOS must own a custom allowlist, formatter, semantic checker, diagnostic
protocol, resource limits, and upgrade policy. Generic `tsc`, ESLint, and
Prettier success cannot admit a robot graph.

Stable diagnostics can become an accidental API: changing rule codes, spans,
ordering, or fix behavior can break automated repair loops. Diagnostic schemas
and formatter output therefore require compatibility and golden tests. Only
mechanical transformations proven to preserve the execution-IR digest may be
auto-applied; safety and policy choices remain explicit. KCL and Starlark are
retained as comparison evidence but are not initial supported frontends.

### Multi-rate and deadline semantics

The HefaOS IR may express requirements that Copper cannot honor directly. The adapter must never approximate these silently. Harmonic rates may be lowered onto an admitted base cycle; non-harmonic or conflicting requirements must either use a supported construct or fail with a source-mapped error. A declared deadline is a contract and observation point, not proof that Linux can meet it.

### Shared-memory constraints

iceoryx2 payloads must be self-contained, bounded, layout-stable, and compatible across participants. Ordinary `String`, `Vec`, heap pointers, and destructors do not belong in a loaned shared-memory payload. Pool exhaustion, dead subscribers, incompatible schemas, process restart, and container shared-memory limits require tests and explicit recovery policy.

Shared-memory peers are trusted processes, not hostile security tenants. Thread-safe port variants may introduce synchronization, and POSIX shared memory does not automatically provide camera-to-GPU/NPU zero copy. Accelerator paths should initially document explicit staging copies.

### Apache Ignite fit

Ignite is optional because it is a distributed SQL/database platform, not a robot command protocol. It should store current desired/observed state, configuration and deployment metadata, indexes, and coordination records only when real scale justifies it. High-rate sensor data belongs in a bounded telemetry pipeline and object/columnar storage.

The current official Ignite client surface does not include a first-party Rust client. A supported-client gateway therefore adds another service boundary. No JVM, database client, transaction, or cluster dependency may be introduced into the critical robot process.

### ROS 2 integration

ROS 2 is an ecosystem boundary, not a transparent transport swap. Every supported bridge must define message mapping, QoS, clock, frame, units, queue behavior, restart behavior, and conversion cost. A bridge is never assumed to preserve HefaOS timing guarantees.

The first bridge must pin one ROS 2 distribution and one mode. `rclrs` currently has API/stability risk; `rmw_zenoh` and `zenoh-bridge-ros2dds` are distinct, non-interchangeable approaches; and cross-distribution type hashes may be incompatible. These constraints require explicit compatibility tests rather than a generic “ROS over Zenoh” claim.

### Functional safety

The v2 architecture can support safety engineering but cannot declare a system safe. Each robot needs hazard analysis, application-specific safe states, verified safety-controller firmware, authenticated reset, mechanical/electrical analysis, and target-standard evidence. Immediate power removal is not universally safe; a suspended load may require controlled braking.

### AI determinism

Recording model identity, inputs, and outputs makes a run explainable but does not make an AI model intrinsically deterministic or safe. A model result is a proposal with provenance, freshness, confidence, and a fallback. Actuation passes through deterministic policy, the software motion gate, and independent safety enforcement.

### Operational scope

The platform still crosses compiler, runtime integration, deployment, observability, safety, AI, and fleet domains. The delivery gates are therefore part of the architecture: features are not allowed to enter the critical path merely because an adapter exists.

## Explicit claim policy

Every user-facing capability must carry one of these statuses:

- **Concept:** a product or research direction with no implementation commitment;
- **Planned:** accepted into a delivery gate but not implemented;
- **Experimental:** implemented without complete target qualification;
- **Implemented:** available and tested for stated environments;
- **Measured:** backed by linked methodology, hardware profile, raw results, and revision;
- **Qualified:** accepted against a named target profile and operational envelope.

“Hard real-time,” “zero copy,” “deterministic,” “safe,” “portable,” and comparative performance language require scoped definitions and evidence. Marketing shorthand does not override this rule.

## Kill criteria

The strategy should pause or pivot if:

- the restricted `*.hefa.ts` agent workflow does not materially improve
  first-pass success, repair iterations, invalid-graph detection, or occasional
  human-review effort versus direct Copper;
- generated applications add unexplained runtime overhead;
- stable Copper integration requires unsupported internals;
- the IR becomes dominated by backend-specific escape hatches;
- pilots value ROS driver breadth more than the HefaOS workflow;
- fleet deployments do not justify Ignite’s operational cost;
- safety claims exceed available engineering and validation capacity.

The defensible moat is a trustworthy workflow for describing, validating, simulating, deploying, inspecting, and operating deterministic robots—not another Rust scheduler.
