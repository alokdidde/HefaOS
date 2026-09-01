# Contributing to HefaOS

HefaOS is currently in an architecture-reset phase. Contributions should help establish the v2 proving slice rather than extend the superseded C++ v1 design.

Read these documents first:

- [Concept](docs/docs/concept.md)
- [Architecture specification](docs/docs/architecture/specification.md)
- [Review and limitations](docs/docs/architecture/limitations.md)
- [Delivery plan](docs/docs/development/roadmap.md)

## Current boundaries

- `runtime/` is legacy C++ scaffolding. Do not add new v2 runtime features there without an accepted migration change.
- Only `*.hefa.ts` is the selected v2 authoring format. It is restricted
  declarative TypeScript without JSX/TSX and is never executed as JavaScript.
- Runtime algorithms and drivers target Rust.
- Copper is the first execution backend; a custom executor is deferred.
- Deterministic data dependencies use explicit typed ports, not a global state store.
- iceoryx2, ROS, AI, telemetry, and fleet services remain outside the critical path unless the specification explicitly admits a bounded boundary.
- Apache Ignite is an optional fleet adapter, not runtime state.
- Unsupported semantics fail closed.
- Present-tense performance and safety claims require linked evidence.

## Workflow

1. Fork and clone the repository.
2. Create a focused branch such as `feature/ir-schema`, `fix/docs-status`, or `test/replay-fixture`.
3. Make the smallest change that advances the active delivery gate.
4. Add tests and evidence appropriate to the change.
5. Update canonical documentation and the changelog when behavior or architecture changes.
6. Open a pull request describing the requirement, boundary, failure modes, and verification.

Use conventional commit messages:

```text
type(scope): concise description
```

Common types are `feat`, `fix`, `docs`, `refactor`, `test`, `perf`, `security`, and `chore`.

## Architecture changes

An architecture proposal should state:

- the user or safety requirement;
- the timing and trust domain;
- data ownership, lifetime, boundedness, and failure behavior;
- backend and target capabilities required;
- compatibility and migration impact;
- security and supply-chain impact;
- tests, measurements, and rollback plan;
- simpler alternatives considered.

A new process boundary must justify its IPC, supervision, resource, restart, and replay cost. A new execution backend must meet Gate 10 of the delivery plan.

## Rust guidance

The planned Rust workspace will require:

- `cargo fmt` and warning-free `cargo clippy`;
- locked dependencies and an explicit minimum supported Rust version;
- unit, property, fuzz, concurrency, and target tests as applicable;
- bounded data structures and no unapproved allocation/blocking in critical phases;
- documented `unsafe`, FFI, thread, lifetime, and panic behavior;
- public APIs with explicit units, clocks, frames, validity, and ownership;
- `panic=abort` and allocation guards for qualified production profiles where specified.

Do not use async runtimes, work stealing, database/network clients, ordinary logging, or filesystem access inside an admitted deterministic control phase.

## Restricted TypeScript authoring guidance

- Name every v2 authoring source `*.hefa.ts`; do not introduce `.tsx`, JSX,
  `.gen.ts`, or ordinary `.ts` as an alternate authoring format.
- Keep declarations immutable, explicit, structural, and inside the documented
  deny-by-default AST allowlist.
- Do not add arbitrary functions, callbacks, classes, effects, timers, dynamic
  imports, runtime I/O, ambient filesystem/environment/network access,
  wall-clock reads, randomness, or direct hardware access to the DSL.
- Treat the HefaOS formatter, linter, and semantic checker as authoritative;
  `tsc`, ESLint, Prettier, and editor diagnostics are only supplemental.
- Preserve stable diagnostic rule codes, project-relative source spans,
  deterministic diagnostic ordering, and versioned JSON/SARIF output.
- Never auto-fix a safety, authority, timing, queue, freshness, resource, or
  fallback choice. A machine fix is available only for already-valid source and
  must be proven to preserve its execution-IR digest; invalid source receives
  repair guidance instead.
- Represent precise identifiers and timestamps through approved constructors
  over canonical decimal strings, never JavaScript `number`.
- Keep source maps and agent/tool provenance detached from execution IR.
- Identical semantic inputs must produce byte-identical canonical IR.

The Gate 1 CLI contract is planned, not implemented by the legacy scaffold:

```bash
hefaos fmt --check
hefaos lint --format sarif --deny-warnings
hefaos check --format sarif --deny-warnings
hefaos build --reproducible
```

## Documentation and claims

Label user-visible work as one of:

- Concept
- Planned
- Experimental
- Implemented
- Measured
- Qualified

Claims involving hard real time, zero copy, determinism, safety, portability, or competitor performance must define their scope and link methodology, hardware, workload, revisions, raw results, and reproduction steps.

## Pull request checklist

- [ ] The change belongs to the active delivery gate.
- [ ] Timing, trust, authority, and failure boundaries remain explicit.
- [ ] Unsupported behavior fails closed.
- [ ] Inputs, queues, memory, and retries are bounded where required.
- [ ] Tests cover error and failure behavior, not only success.
- [ ] Generated artifacts remain deterministic where applicable.
- [ ] `*.hefa.ts` changes satisfy the restricted AST and semantic rules; generic TypeScript lint alone is not used as evidence.
- [ ] Diagnostics remain stable and every automatic fix is semantics-preserving; no safety or policy decision is invented.
- [ ] Documentation and status labels are accurate.
- [ ] Dependency, license, security, and migration impacts are documented.
- [ ] No formatter, generator, linter, schema, or test failure is swallowed.

## License

Contributions are licensed under the repository's MIT License.
