# HefaOS architecture

This document defines architectural responsibilities and invariants. It does not describe a current implementation. Normative requirements live in the [specification](specification.md), and binding choices live in accepted [architecture decisions](decisions/0001-greenfield-rust-reset.md).

## System boundary

HefaOS owns the path from application intent to locally supervised physical action, plus the evidence needed to explain that path. Hardware drivers, simulators, model runtimes, authoring tools, storage, networking, and fleet services may integrate at explicit boundaries; none is implicitly part of the trusted local control path.

## Execution islands

An execution island is a fault, scheduling, and data-delivery boundary. Within an island:

- exactly one authority owns scheduling;
- exactly one authority owns delivery semantics for each connection;
- computations communicate through typed ports;
- each computation's mutable state has one explicit owner;
- temporal, queue, memory, and failure behavior is bounded by declared contracts.

The architecture does not yet decide whether an island is implemented by an owned executor, an adapted third-party executor, or another mechanism. It also does not decide the process, thread, IPC, or asynchronous-runtime mapping. Those mechanisms must preserve the ownership invariant rather than create competing schedulers or delivery paths.

## Application contracts

Applications may express periodic, data-driven, and goal-driven work. Timing is defined per relevant computation and connection, including the properties needed to reason about periods or triggers, phase, deadline, sampling, freshness, queueing, overflow, and fallback.

There is no global selected frequency. Admission considers the interacting set of contracts and available resources. Work that cannot be admitted safely must be rejected with an observable reason.

## Semantic dataflow

The architecture keeps these meanings distinct:

1. observation — a reported fact from a source at a stated time;
2. estimate — an inferred current state with provenance and uncertainty;
3. prediction — a possible future state under stated assumptions;
4. proposal — a request for consideration, not authority to actuate;
5. reference — an admitted target for a controller;
6. command — an instruction eligible for the hardware boundary;
7. outcome — observed progress or result, including partial physical progress.

Typed ports must make invalid semantic substitution difficult. Concrete Rust type names and serialization formats remain open.

## Motion authority

Physical action follows four separate responsibilities:

`proposal -> admission -> command -> independent protection`

- Proposal producers may be classical, learned, remote, or operator-driven.
- Admission checks current authority, state, constraints, freshness, resources, and policy.
- Command production translates an admitted reference into bounded device-facing intent.
- Independent protection enforces final limits without relying on the proposer or controller being correct.

The declared hardware actuation boundary is the sole path by which a request may cause or continue physical motion. Operator, manual, maintenance, and recovery paths may have distinct authority policies, but they cannot bypass admission, command production, or independent protection. For the failure model selected by the first operational profile, independent protection must remain effective if the application or control runtime stalls or crashes. This requirement selects an outcome and fault boundary, not a hardware design.

Cancellation, epoch changes, stale data, partial execution, and loss of dependencies must have explicit semantics. Unknown or unsafe authority fails closed.

## Control and best-effort services

Local sensing, estimation required for control, admission, command production, and protection form the local control path. AI inference, network access, storage, fleet coordination, analytics, and rich telemetry are best-effort extensions unless a later safety case explicitly promotes a dependency.

Backpressure or failure in a best-effort extension must not block, starve, reorder, or silently alter the admitted local control path. A classical-only configuration must remain complete and operable.

## Observability and replay

Operational telemetry and replay evidence are separate products:

- telemetry supports live diagnosis and may be sampled or lossy according to its contract;
- replay evidence preserves the ordered inputs, decisions, versions, epochs, and outcomes required for a declared reproducibility claim.

Every bounded failure needs an owner-visible signal. Evidence must identify its scope and must not imply stronger determinism, timing, safety, or compatibility than was tested.

## Compatibility boundary

Stable contracts should be smaller than implementations. The architecture expects a versioned intermediate contract between authoring intent and execution, but its schema, encoding, generation path, and public surface are undecided. Compatibility promises begin only when an ADR defines the contract and a conformance artifact verifies it.

## Open architecture decisions

The following choices are intentionally unresolved:

- first robot, use case, and deployment profile;
- boundary of Rust ownership and any external authoring surface;
- stable intermediate representation and versioning rules;
- owned versus third-party execution backend;
- process, thread, IPC, and asynchronous-runtime model;
- simulator and hardware integration boundary;
- model runtime and learned-component isolation;
- public APIs, SDKs, DSLs, code generation, and concrete type names;
- evidence storage format and replay implementation.

The [roadmap](roadmap.md) orders these decisions and requires a focused probe only where documentary reasoning cannot retire the uncertainty.
