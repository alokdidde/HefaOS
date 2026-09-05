# ADR 0001: Greenfield Rust-first reset

- Status: Accepted
- Date: 2026-09-05
- Scope: repository baseline and work order

## Context

The previous repository mixed early C++, Rust, TypeScript, testbench, tooling, and Copper integration work with evolving product ideas. Most of that implementation was incomplete, and its structure risked turning accidental choices into architectural constraints. The 2026-09-05 redesign proposal supplies the updated concept, but it intentionally includes illustrative mechanisms and unresolved options.

## Decision

Reset the active repository to a documentation-only, Rust-first architecture baseline. Preserve the redesign proposal verbatim as non-normative input, distill the accepted invariants into canonical documents, and remove previous implementation and implementation-specific infrastructure from the active tree.

Resolve architecture before implementation. After architecture readiness, build one narrow conformance probe for one named uncertainty. Do not begin with a complete testbench.

Canonical authority is:

`specification > accepted ADRs > architecture > concept`

The roadmap controls sequence and status. Proposals do not carry authority.

## Active-tree allowlist

The reset is complete when the active tree contains exactly:

```text
.editorconfig
.gitignore
AGENTS.md
CONTRIBUTING.md
LICENSE
README.md
docs/architecture.md
docs/bootstrap.md
docs/concept.md
docs/decisions/0001-greenfield-rust-reset.md
docs/proposals/hefaos-redesign-2026-09-05.md
docs/roadmap.md
docs/specification.md
```

## Preserved source integrity

The proposal source is preserved at `docs/proposals/hefaos-redesign-2026-09-05.md` with SHA-256:

```text
5dff93305ce355a72a8b9f62f5ea37477ddffe4c2b1915a85faaedc577e1e81f
```

Its presence records design input, not acceptance of its suggested libraries, APIs, type names, module names, timing examples, or implementation plan.

## Recovery point

Tracked pre-reset work is recoverable from:

- commit `bde7adf` (`chore(archive): preserve pre-greenfield workspace`);
- branch `codex/archive/pre-greenfield-2026-09-05`;
- annotated tag `pre-greenfield-v2-reset-2026-09-05`.

The recovery point does not claim to contain untracked raw evidence, generated build output, or crash logs. Those are local artifacts, not part of the active versioned baseline.

## Invariants carried forward

- Rust owns the intended systems boundary; its precise scope and public surface remain undecided.
- Dataflow uses typed ports and task-private state with explicit, bounded ownership and resources.
- Applications define periodic, data-driven, and goal-driven temporal contracts; there is no global selected frequency.
- Each execution island has one scheduling authority and one delivery-semantics owner per connection.
- Local control remains independent of AI, network, storage, analytics, and fleet availability.
- Physical action separates proposal, admission, command production, and independent protection.
- Classical control is a complete product profile; learned and hybrid extensions cannot weaken safety.
- Observation, estimate, prediction, proposal, reference, command, and outcome remain semantically distinct.
- Safety-relevant uncertainty fails closed.
- Claims remain limited to preserved reproducible evidence.

## Choices not made by this decision

This ADR does not select Copper or an owned executor, process or IPC topology, async runtime, simulator, model runtime, authoring language or DSL, public Rust API, SDK shape, serialization format, or concrete type names.

## Consequences

The active branch loses direct access to the previous code layout, but the recovery point retains its tracked history. Near-term work produces decisions rather than features. This adds a deliberate architecture gate and avoids spending effort on a complete testbench before its contracts are stable.

Any useful previous behavior must be justified against the new specification rather than copied by default. Future compatibility and performance claims start from new, pinned evidence.

## Revisit trigger

Revisit this decision only if recovery validation fails, the canonical documents cannot express a required product property, or the first operational profile demonstrates that Rust cannot own the intended systems boundary. Record any change in a superseding ADR.
