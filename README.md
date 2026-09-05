# HefaOS

HefaOS is a Rust-first robotics systems project being rebuilt from a documentation-only greenfield baseline.

The project is currently defining architecture. No previous implementation, testbench, dependency choice, public API, performance result, or compatibility claim is part of the supported design unless it is accepted again through the current decision process.

## Start here

Read these documents in order:

1. [Concept](docs/concept.md)
2. [Architecture](docs/architecture.md)
3. [Specification](docs/specification.md)
4. [Greenfield reset decision](docs/decisions/0001-greenfield-rust-reset.md)
5. [Roadmap](docs/roadmap.md)

Use [the architecture bootstrap](docs/bootstrap.md) to begin a new design session. The preserved [redesign proposal](docs/proposals/hefaos-redesign-2026-09-05.md) is source material, not an accepted specification.

## Authority

When documents conflict, authority descends in this order:

1. specification;
2. accepted architecture decision records (ADRs);
3. architecture;
4. concept.

The roadmap controls work order and completion status. Proposals provide input only.

## Current boundary

The greenfield baseline is not permission to rush into code. Architecture decisions come first. The first implementation will be one narrow conformance probe chosen to retire a named uncertainty. A complete testbench is deliberately deferred until stable contracts exist for it to test.

See [Contributing](CONTRIBUTING.md) before changing the repository.
