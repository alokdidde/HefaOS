# Execution backend capability and conformance contract

**Version:** 1.0 draft
**Status:** Planned — canonical draft
**Scope:** Vendor-neutral execution-semantic contract; not a backend API, task algorithm specification, performance claim, or authorization to replace Copper.

## Decision and boundary

HefaOS extends the canonical backend-capability and backend-conformance boundary in the [architecture specification](specification.md#44-capability-negotiation). It owns declarations, validation, lowering verdicts, diagnostics, evidence references, and conformance results. A backend owns its implementation and may claim only the capabilities and target profiles that its evidence supports.

Copper is the sole initial execution backend. A capability declaration does not create a second scheduler, local bus, recorder, global runtime state, or safety authority. It decides whether validated HefaOS IR can be generated for a named backend and target profile.

This document does not specify a Rust trait, wire format, scheduler algorithm, or replacement executor. Copper integration pain alone is insufficient to build a native executor. The [Gate 10 admission rule](../development/roadmap.md#gate-10-additional-execution-backend) requires a concrete user requirement, missing Copper semantic, attempted upstream extension (or a documented reason it is unsuitable), capability mapping, conformance evidence, and funded lifecycle, replay, security, and safety ownership.

## Current observed baseline: Copper only

The only observed backend evidence is the direct hand-written Copper spike at Copper v1.1.1, commit fc2ebc4fe3583d1f433b75898ad7c9e4dd9e6af2. Its frozen acceptance exercises a static graph, lifecycle, typed local messages, named resources, source simulation, bounded recording, and replay through public APIs; see [Gate 0 Copper spike — frozen acceptance](../development/gate-0-copper-spike.md) and the [raw evidence record](../development/evidence/gate-0-copper-v1.1.1-8b79968.md).

This is **experimental, local-only evidence**. The reviewed raw bundle is not a durable clone-portable archive, Gate 0 remains incomplete, and the evidence does not qualify hardware timing, production replay, zero-copy IPC, safety, or a target profile. The virtual workload and exclusions are fixed separately by [Gate 0 scope and fixture lock](../development/gate-0-scope-fixture-lock.md). The source pin is evidence, not a description: capability or backend decisions MUST compare declared requirements with executed, pinned evidence and its verdicts, never compare descriptions in place of evidence.

Copper's observed evidence supplies no portability baseline and does not make its current cu_iceoryx2_bridge admissible on a control edge. Gate 0 rejects that bridge pending bounded schema, queue, epoch, pool, failure, and replay policies. The specification still requires one supported Copper bridge when it meets the contract, or one HefaOS iceoryx2 adapter otherwise; two transports must not carry the same logical edge.

## Island ownership invariant

An execution island is one independently supervised process with one executor, local task state, a resource budget, a restart epoch, and declared ingress and egress edges. For each execution island, there MUST be exactly:

1. one executor and one owner of scheduling/release decisions;
2. one local typed delivery path for each logical in-island edge; and
3. one admitted capture path for replay-required execution evidence.

No backend integration may add a second scheduler, event bus, recorder, or mutable global state for the same island or logical edge. Best-effort telemetry is a separate, bounded observation flow and cannot substitute for, backpressure, or silently share authority with admitted capture. Cross-process transport is a declared ingress or egress boundary, not an extra executor. All actuator output continues through the local software motion gate and the independent safety controller; neither a backend nor a recorder gains actuator authority.

## Versioned capability declaration

Every backend/target declaration and every IR requirement uses a stable, namespaced capability ID and a contract version. The declaration is a machine-readable, versioned artifact associated with the backend build, target profile, and evidence bundle. The following is a non-claim schema shape: it is intentionally non-admissible, names no current backend profile, and cannot be read as an exact Copper lowering. Field names remain illustrative until the Gate 1 schema is published.

~~~yaml
id: org.hefaos.execution.static-graph
contract_version: 1.0.0
backend: <backend-id>
backend_version: <pinned-backend-version>
target_profile: <target-profile-id>
declaration:
  required: true
  semantics: static typed graph; explicit ports; no undeclared reads
lowering:
  disposition: unsupported # exact | qualified-lowering | unsupported
  qualification: [] # required evidence, constraints, and semantic delta
maturity: planned # planned | experimental | implemented | measured | qualified
boundedness:
  queues: declared
  memory: declared
  cpu_or_device_budget: declared
failure:
  policy: fail-closed
  diagnostic_id: backend.capability.unsupported
replay_and_evidence:
  required_capture: true # true | false
  source_classification: deterministic | captured-nondeterministic | unavailable
  evidence_refs: [<immutable-evidence-reference>]
conformance:
  profile: execution-backend/v1
  cases: [<conformance-case-id>]
~~~

Exact means that the backend preserves the required semantic without an undeclared change. Qualified lowering means that the declaration names every precondition, semantic delta, target constraint, and evidence reference; the compiler may use it only when all are satisfied. Unsupported is a stable fail-closed result, not a request to approximate, emulate silently, or move the requirement to hidden runtime state. Missing, malformed, incompatible-major, or unknown mandatory declarations are also unsupported.

Capability IDs below form the initial execution-backend/v1 namespace. A profile MAY split a capability into finer IDs, but it MUST retain the parent semantic and conformance reference.

| Capability ID | Required declaration |
|---|---|
| org.hefaos.execution.static-graph | Static graph identity, typed ports, topology, task/edge IDs, and proof that no undeclared read or mutable global control state is introduced. |
| org.hefaos.execution.lifecycle-checkpoint-restart | Construct/start/process/stop states; checkpoint/state serialization; restart behavior; epoch increment/fencing; partial-progress disposition. |
| org.hefaos.execution.time-rate-deadline | Monotonic clock, release period/phase, input sampling, deadline/overrun observation, harmonic-base-cycle lowering constraints, and unavailable semantics. |
| org.hefaos.execution.local-message-queue-freshness | Message schema/type IDs, local delivery ownership, finite queue/history, overflow, ordering, source sequence, age/clock, invalid and stale-input policy. |
| org.hefaos.execution.named-resource-budget | Named CPU, memory, device, pool, and placement budgets; admission checks; allocation/blocking constraints; exhaustion behavior. |
| org.hefaos.execution.fault-partial-progress | Error, panic, timeout, cancellation, crash, overrun, unavailable output, partial turn, and recovery/safe-state policy. |
| org.hefaos.execution.simulation | Virtual/external clock, source injection, seed/environment identity, simulated I/O boundary, and simulation-versus-hardware claim scope. |
| org.hefaos.execution.capture-replay | Required capture set, task-state checkpoint/restore, replay classification and equality contract, capture bounds, loss policy, and evidence retention. |
| org.hefaos.execution.observability | Stable diagnostic/event IDs, counters, trace fields, loss/miss/fault visibility, clock/epoch correlation, and best-effort isolation. |
| org.hefaos.execution.cross-process-ipc | Process placement, schema/service identity, bounded payload/alignment, queue/pool/loan ownership, restart/stale handling, copy boundaries, and one-transport rule. |
| org.hefaos.execution.provenance | IR, component, model, configuration, backend, target, toolchain, artifact, and evidence digests; schema versions; reproducibility disposition. |
| org.hefaos.execution.motion-gate-safety | Software motion-gate crossing, permit/status/epoch freshness, actuator authority, safe-state behavior, and independent-controller boundary. |

## Stable failure and observability contract

The compiler MUST emit a stable diagnostic ID, a normalized project-relative path and source span, capability ID and version, backend and target profile, lowering disposition, and a reason for every failed negotiation. If the rejected requirement has no user-authored source, it MUST instead use the reserved normalized project-relative synthetic/generated path `__hefaos_generated__/backend/<semantic-node-id>` and a deterministic zero-width range encoded in the same normalized source-span representation as user-authored diagnostics. The path is bound to the semantic IR node; absence of a path or range is not permitted. At minimum, backend.capability.unsupported, backend.capability.unqualified-lowering, backend.capability.incompatible-version, and backend.capability.missing-evidence are reserved in this contract. Their meaning and fail-closed action may not change within a major contract version.

At runtime, backend-originated events needed to explain a run MUST include the same capability identity where relevant, island ID, restart epoch, monotonic timestamp with unit and clock-domain identity, source sequence when applicable, declared failure policy, and the IR and artifact digests. Required counters include deadline/overrun observations, queue and pool exhaustion, drops/replacements, stale/invalid rejections, checkpoint/restore outcomes, capture loss, replayability verdict, and motion-gate/safety rejection. Counters and best-effort logs are observations; they do not authorize a command or repair a failed capability negotiation.

| Condition | Required result |
|---|---|
| Unknown mandatory capability, incompatible major version, unsupported semantic, or unqualified lowering | Reject build/deployment before generation; emit a stable diagnostic. |
| Missing bound, resource admission, required evidence reference, or safety declaration | Reject build/deployment; do not assume an unbounded/default behavior. |
| Runtime task error, panic, timeout, overrun, stale/invalid input, IPC incompatibility, or resource exhaustion | Apply the declared fail-closed fallback/safe-state policy; report the event and partial-progress disposition. |
| Required capture loss or unavailable required task state | Mark the run non-replayable or apply its declared recording fault policy; never label it replay-complete. |
| Missing, stale, incompatible, or rejected safety permit/status | Reject motion at the software gate and preserve the independent safety boundary. |

## Compatibility, migration, and additional-backend admission

Capability IDs are immutable. An additive optional field is compatible only when older consumers can ignore it without changing execution semantics. A new mandatory field, changed semantic, changed fail-closed action, or changed evidence/equality requirement requires a new major capability or contract version and an explicit migration. A migration MUST identify the old and new IDs/versions, preserve the historical artifact's declared semantics, regenerate the IR/artifact and evidence, and re-run the applicable conformance profile. No deployment may infer compatibility from a backend name or marketing version.

The compiler admits a backend only when each required capability has a compatible version and an exact or applicable qualified-lowering verdict, all declared bounds and target constraints are met, and its conformance and evidence references resolve to successful, applicable verdicts identity-bound to the pinned backend revision, target-profile digest, and IR/deployment-artifact digests. Experimental evidence proves only its stated experimental scope; it does not admit deployment or qualify a target. Deployment admission additionally requires the applicable qualified verdict for the named target profile and artifact. Every emitted negotiation diagnostic MUST be in deterministic order. At the parent-declared diagnostic-resource limit, the compiler MUST truncate that ordered result deterministically and emit the stable diagnostic.limit-exceeded diagnostic; it MUST not report an unbounded set of failures. Any one mandatory failure denies generation.

An additional or native backend enters Gate 10 only with the exit evidence in the [delivery plan](../development/roadmap.md#gate-10-additional-execution-backend): a named user workload and unmet Copper semantic, upstream-extension result or documented unsuitability, capability mapping and fail-closed diagnostics, full conformance suite, owned lifecycle/security/replay/safety burden, and no regression to the proven Copper path. Revisit this contract when evidence shows a material unexpressible semantic or target requirement, Copper's public contract cannot safely support a required lowering, a qualified target exposes a repeatable conformance failure, or a capability migration changes the stated semantic. Integration inconvenience, API style, implementation preference, or an unmeasured performance intuition is not a revisit trigger by itself.

## Conformance profile and negative tests

Execution-backend/v1 is a planned conformance profile. A backend is not supported merely because it supplies a declaration or passes a happy-path demo. Each row requires a pinned backend/target/artifact identity, command and successful applicable verdict record, and retained evidence appropriate to its replay classification. The current Copper evidence is limited to the experimental Gate 0 scope above; these tests are the admission matrix for later supported profiles, not a claim that they have all run or deployment qualification.

| Capability | Required conformance case | Mandatory negative test / fail-closed verdict |
|---|---|---|
| Static graph and ports | Compile a fixed typed graph; inspect stable task/edge IDs and verify only declared ports carry control data. | Unknown port/type, unsafe or unadmitted cycle, undeclared read, or global-state access rejects generation. An admitted feedback loop MUST declare its explicit delay or state boundary. |
| Lifecycle, checkpoint, restart epoch | Start/process/stop; checkpoint and restore a declared state; restart and verify epoch fencing. | Missing checkpoint support when required, stale prior-epoch event, or ambiguous partial turn is rejected or transitions by declared policy. |
| Time, rates, deadlines | Exercise declared release/phase, harmonic lowering, monotonic clock, and overrun observation. | Non-harmonic/ambiguous join, unavailable clock, or unsupported deadline semantics rejects; a measured loop interval is not proof. |
| Local messages, queues, freshness | Verify schema, order, capacity, overflow, age, and invalid/stale fallback for one in-island edge. | Queue overflow without declared policy, reorder on critical edge, stale or malformed sample fails closed. |
| Named resources and budgets | Admit a graph within named CPU/memory/device/pool budgets and record use/exhaustion. | Missing or exceeded budget, allocation/blocking violation, or pool exhaustion denies admission or applies declared fault policy. |
| Faults and partial progress | Inject task error, panic, timeout, crash, and unavailable output; record the safe/fallback action. | Continued actuator authority after an undeclared or unhandled partial-progress fault fails conformance. |
| Simulation | Run a source-injected virtual-clock graph with pinned seed/model/environment identity. | Missing simulation identity or labelling simulation as hardware qualification fails the claim/conformance verdict. |
| Capture and replay | Capture required inputs, ticks, drops, faults, and declared task state; replay against its named equality contract. | Dropped required evidence, absent required state, or equality without pinned artifact/profile marks the run non-replayable. |
| Observability | Verify stable diagnostics and correlation of island, epoch, time, sequence, IR/artifact digest, and loss/fault counters. | Missing mandatory fields, unstable diagnostic ID, or best-effort telemetry used as replay evidence fails. |
| Cross-process IPC | Exercise one bounded service/schema, queue/pool exhaustion, borrower/process death, restart, stale samples, and explicit copy/loan behavior. | Incompatible schema, unbounded payload/queue/pool, duplicate transport for one edge, or unspecified restart behavior rejects admission. |
| Provenance | Rebuild from complete pinned inputs and bind IR/artifact/backend/target/evidence digests to the result. | Missing or mismatched digest, unknown toolchain/schema version, or unverifiable evidence reference rejects qualification. |
| Motion gate and safety | Verify every actuator proposal crosses the motion gate with fresh permit/status/epoch feedback and retains independent-controller authority. | Bypass, stale/missing/incompatible permit, or safety-status rejection blocks motion; a backend cannot override it. |

The profile compares each backend with its declared semantic contract. It does not assume identical output across backends, and it does not compare prose descriptions in lieu of capability evidence.

## Non-goals and review rule

This contract intentionally does not authorize a native HefaOS executor, replacement backend, transparent backend portability, arbitrary callback semantics, a second local transport, a global state store, or a generic replay or telemetry system. It exists to keep future choices evidence-bound and preserve the Copper path while HefaOS completes the earlier delivery gates.
