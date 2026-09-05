# New-session architecture bootstrap

Copy the prompt below into a new session opened at the HefaOS repository root.

```text
We are rebuilding HefaOS from a documentation-only greenfield baseline. Treat the repository as architecture source material, not as an implementation to continue. Do not restore or imitate deleted code.

Read these files completely before acting:
1. docs/concept.md
2. docs/architecture.md
3. docs/specification.md
4. docs/decisions/0001-greenfield-rust-reset.md
5. docs/roadmap.md
6. docs/proposals/hefaos-redesign-2026-09-05.md
7. AGENTS.md

Authority is: specification > accepted ADRs > architecture > concept. The roadmap controls work order and status. The proposal is non-normative input only; do not silently accept its illustrative technology choices, APIs, type names, target numbers, or module layout.

This session is architecture-only. Do not write product code, testbench code, tools, dependency manifests, generated bindings, examples, implementation-specific CI, SDKs, DSLs, or public APIs. Do not design a complete testbench. Architecture must become ready first; afterward a separate session will build one narrow conformance probe for one explicitly recorded uncertainty.

Goal: make the smallest foundational decisions required before any Rust implementation:
- the first robot or simulator-backed use case, deployment profile, hazards, and non-goals;
- the boundary Rust owns and whether any external authoring surface is needed;
- the stable, versioned contract between authoring intent and execution;
- execution-island boundaries, scheduling ownership, delivery ownership, and a fair evaluation method for candidate backends;
- application-defined periods or triggers, applicable clock domains, phase, deadline, sampling, freshness, queueing, overflow, admission, overload, and fallback, with no global selected rate; each cross-domain mapping needs identity and version, uncertainty or error bounds, reset and reconnect semantics, and a fail-closed result when freshness, ordering, or deadlines cannot be decided within bounds;
- separation of local control from best-effort AI, network, storage, analytics, and fleet services;
- proposal, admission, reference, command, and independent-protection authority, with one hardware actuation boundary that no operator, manual, maintenance, or recovery motion path can bypass;
- identity, epochs, cancellation, restart, stale data, partial physical progress, and fail-closed outcomes;
- operational telemetry versus replay evidence, compatibility, versioning, and claim boundaries;
- the single highest-risk uncertainty that the first narrow probe must answer.

Preserve these invariants:
- typed ports, computation-private state, and explicit bounded ownership, lifetimes, queues, and resources;
- one scheduling authority per execution island and one delivery-semantics owner per connection;
- a classical-control-only system is a complete supported product profile;
- learned or hybrid extensions cannot bypass admission/protection or become a safety or liveness dependency of local control;
- independent protection remains effective for the selected deployment failure model when the application or control runtime stalls or crashes, without assuming a particular hardware design;
- observation, estimate, prediction, proposal, reference, command, and outcome are distinct meanings;
- safety-relevant uncertainty fails closed;
- no correctness, determinism, timing, performance, compatibility, or safety claim without scoped raw reproducible evidence.

Ask questions only when the answer materially changes a system boundary, invariant, safety policy, or the first operational profile. Otherwise state a conservative assumption and its revisit trigger.

Work on roadmap artifacts A1 through A6 in order. Before each artifact, freeze its documentary question, acceptance criteria, inputs, environment, correctness/safety/security guards, metrics or decision thresholds, and clean-checkout verification. Keep artifacts independently judgeable. Use one drafter and a separate fresh-context harsh critic; the critic must inspect the actual final diff and reject correctness, safety, security, determinism, maintainability, performance, or unsupported-claim defects. Resolve every actionable finding without weakening a frozen criterion.

For each accepted decision, record an ADR containing:
- decision, status, confidence, and owner;
- boundary, contracts, and invariants;
- failure, recovery, and fail-closed behavior;
- observability and evidence requirements;
- compatibility, migration, rollback, and security consequences;
- alternatives considered and why they were rejected;
- unknowns, assumptions, and an objective revisit trigger.

You may edit canonical documentation and add ADRs only when needed to record accepted architecture. Do not create implementation files.

Finish by producing:
1. the updated architecture documents and accepted ADRs;
2. a readiness checklist mapped to every item under "Architecture readiness" in docs/specification.md;
3. a list of unresolved choices and risks;
4. exactly one frozen first-probe specification;
5. a separate, self-contained prompt for a later session to implement and evaluate only that probe.

Stop before implementation.
```
