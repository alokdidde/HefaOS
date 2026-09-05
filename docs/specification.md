# HefaOS foundational specification

This specification is normative for the greenfield architecture. `MUST`, `MUST NOT`, `SHOULD`, and `MAY` carry their usual requirements meaning. These requirements constrain future implementations; they are not claims that an implementation currently exists.

## S1. Ownership and dataflow

- **S1.1** Application computations MUST exchange values through explicit typed ports.
- **S1.2** Mutable computation state MUST have one explicit owner and MUST NOT be shared through undeclared channels.
- **S1.3** Port contracts MUST distinguish at least observation, estimate, prediction, proposal, reference, command, and outcome semantics where those concepts occur.
- **S1.4** A proposal MUST NOT be accepted as a hardware command without admission and command-production steps.
- **S1.5** Lifetimes, queues, memory use, and other exhaustible resources MUST be bounded or rejected at admission.

## S2. Execution ownership

- **S2.1** Every execution island MUST have exactly one scheduling authority.
- **S2.2** Every connection within an island MUST have exactly one delivery-semantics owner.
- **S2.3** An integration MUST NOT introduce a second scheduler or delivery path that can violate the island's ordering, timing, queue, cancellation, or failure contract.
- **S2.4** Cross-island communication MUST declare ownership transfer, ordering, queueing, overflow, freshness, cancellation, and failure behavior.

## S3. Temporal contracts

- **S3.1** The application MUST be able to define periodic, data-driven, and goal-driven work without a system-wide fixed frequency.
- **S3.2** A relevant temporal contract MUST declare its applicable clock domain, trigger or period, and all applicable phase, deadline, sampling, freshness, queueing, overflow, and fallback behavior.
- **S3.3** A mapping between clock domains MUST declare its identity and version, uncertainty or error bound, and reset and reconnect semantics.
- **S3.4** Scheduling admission MUST evaluate interacting temporal and resource contracts against the declared environment.
- **S3.5** If freshness, ordering, or deadline satisfaction cannot be decided within declared clock-mapping bounds, the affected operation MUST fail closed with a stable, observable reason.
- **S3.6** Work that cannot be admitted within its declared bounds MUST fail closed with a stable, observable reason.
- **S3.7** An implementation MUST NOT infer safety or feasibility merely because a requested rate falls inside a nominal frequency range.

## S4. Local control independence

- **S4.1** The admitted local control path MUST continue to provide its declared safe behavior without AI inference, network connectivity, remote control, storage services, analytics, or fleet coordination.
- **S4.2** A classical-control-only deployment MUST be a complete supported product profile.
- **S4.3** Learned and hybrid components MAY produce estimates, predictions, or proposals, but MUST NOT bypass admission or independent protection.
- **S4.4** Failure, delay, overload, or backpressure in a best-effort extension MUST NOT block, starve, reorder, or silently change the admitted local control path.

## S5. Motion authority and safety

- **S5.1** Motion authority MUST be separated into proposal, admission, command production, and independent protection responsibilities.
- **S5.2** Every request capable of causing or continuing physical actuation, including operator, manual, maintenance, and recovery requests, MUST cross the sole declared hardware actuation boundary and MUST remain subject to admission, command production, and independent protection.
- **S5.3** Admission MUST validate authority, relevant state, constraint satisfaction, input freshness, resource feasibility, and applicable policy.
- **S5.4** Independent protection MUST enforce the final declared device limits without trusting the proposal source or controller to be correct, and MUST remain effective under the application- and control-runtime stall and crash cases selected by the A1 deployment failure model.
- **S5.5** Epoch changes, cancellation, stale inputs, partial physical progress, restart, and dependency loss MUST have explicit state transitions and externally visible outcomes.
- **S5.6** Missing, contradictory, stale, or unverifiable safety-relevant state MUST fail closed.
- **S5.7** Recovery MUST NOT report cancellation or rollback of physical work that has already occurred; partial progress MUST remain observable.

## S6. Failure, security, and observability

- **S6.1** Every failure MUST have a defined owner, containment boundary, externally observable signal, and safe fallback or terminal state.
- **S6.2** Inputs crossing a trust boundary MUST be authenticated or otherwise admitted according to an explicit policy before they can affect motion authority.
- **S6.3** Resource exhaustion, malformed input, version mismatch, and unavailable dependencies MUST have deterministic admission or failure behavior within declared bounds.
- **S6.4** Safety-relevant decisions MUST carry enough identity, version, time, epoch, and provenance information to attribute their inputs and outcome.
- **S6.5** Diagnostic telemetry MAY be sampled or lossy only when that behavior cannot invalidate a claimed replay or safety property.

## S7. Evidence and claims

- **S7.1** A correctness, determinism, timing, performance, compatibility, or safety claim MUST identify the tested version, platform, configuration, workload, metric, threshold, reproduction command, and preserved raw output.
- **S7.2** A claim MUST NOT extend beyond the environments and behaviors demonstrated by its raw evidence.
- **S7.3** Replay evidence and operational telemetry MUST have distinct contracts.
- **S7.4** A reference comparison MUST use fetched, pinned, supported source and preserved raw reference output; a prose description is not a reference result.
- **S7.5** Frozen acceptance thresholds and valid tests MUST NOT be weakened or removed to obtain a pass.

## S8. Compatibility and evolution

- **S8.1** Stable contracts MUST be explicitly versioned before compatibility is claimed.
- **S8.2** An incompatible contract change MUST define detection, migration or rejection behavior, and rollback consequences.
- **S8.3** Implementation mechanisms MUST remain replaceable behind the smallest stable contract practical.
- **S8.4** Proposal examples, illustrative APIs, technology names, and type names MUST remain non-normative until accepted by an ADR.

## Architecture readiness

Implementation may begin only when accepted decisions define:

1. the first use case and deployment profile;
2. the Rust ownership boundary and authoring boundary;
3. the versioned contract between authoring intent and execution;
4. execution-island ownership and a backend evaluation method;
5. temporal admission, queueing, overload, and fallback semantics;
6. motion authority, epochs, cancellation, partial progress, and fail-closed behavior;
7. telemetry, replay evidence, compatibility, and claim boundaries;
8. the single uncertainty to be answered by the first narrow conformance probe.

Meeting readiness authorizes only that probe, not a full product or complete testbench.
