# Contributing to HefaOS

HefaOS is in an architecture-first greenfield reset. Contributions must preserve the distinction between an intended contract and an implemented or measured fact.

## Before changing anything

1. Read [the documentation authority rules](README.md#authority).
2. Locate the earliest incomplete artifact in [the roadmap](docs/roadmap.md).
3. Freeze that artifact's acceptance criteria, workload, environment, correctness guards, metrics, thresholds, and clean-checkout reproduction command before doing the work.
4. Keep the change to the smallest independently judgeable artifact.

During the architecture phase, do not add product code, dependency manifests, generated bindings, examples, testbench code, implementation-specific CI, or public APIs.

## Decisions

Record a durable ADR for a choice that constrains future implementation. An ADR must state:

- the decision and its confidence;
- the boundary and owner;
- contracts and invariants;
- failure and recovery behavior;
- observability and evidence requirements;
- compatibility, migration, and rollback consequences;
- rejected alternatives;
- remaining unknowns and a revisit trigger.

Do not silently turn a proposal's illustrative names, technology examples, or target numbers into accepted design.

## Evidence and claims

Claims are limited to what preserved raw evidence demonstrates. A reproducible claim identifies the version, platform, configuration, workload, metric, threshold, command, and raw output. A passing description without its raw reference output is not evidence.

Do not weaken an accepted check or threshold to make a change pass. Apply actionable review findings and rerun the full artifact check on the final tree.

## Review

Every substantive artifact needs an implementation or document author and a separate fresh-context critic. The critic inspects the actual final diff and evidence for correctness, safety, security, determinism, maintainability, and performance implications. An artifact advances only after all valid findings are resolved.
