# HefaOS agent instructions

HefaOS is at a documentation-only, architecture-first reset.

## Authority

Read, in order:

1. `docs/concept.md`
2. `docs/architecture.md`
3. `docs/specification.md`
4. accepted records in `docs/decisions/`
5. `docs/roadmap.md`

When content conflicts, use this precedence:

`specification > accepted ADRs > architecture > concept`

The roadmap controls sequence and status. Files in `docs/proposals/` are non-normative input and must not be treated as accepted decisions.

## Current phase

Do not write product code, a testbench, tools, dependency manifests, implementation-specific CI, examples, generated files, or public APIs until the architecture readiness criteria in the roadmap pass. Do not restore or imitate deleted implementation from Git history unless an accepted artifact explicitly requires historical investigation.

Resolve architecture first. Then build one narrow conformance probe that answers one recorded uncertainty. Do not build a complete testbench first.

## Required method

- Work on the earliest incomplete roadmap artifact.
- Before work, freeze its acceptance criteria, workload, environment, correctness guards, metrics, thresholds, and clean-checkout reproduction command.
- Use the smallest independently judgeable artifact.
- For a substantive artifact, use a separate author and fresh-context critic. The critic must inspect the actual final diff and available evidence.
- Resolve every actionable finding and rerun the frozen checks without weakening them.
- Record only evidence-scoped implementation, compatibility, determinism, safety, and performance claims.

Architecture work must preserve the invariants in `docs/specification.md` and leave explicitly open choices open until an ADR accepts them.
