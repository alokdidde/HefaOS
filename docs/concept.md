# HefaOS concept

HefaOS is intended to make robot behavior explicit, bounded, inspectable, and safe to evolve. It is Rust-first because ownership, types, and controlled resource use fit the systems boundary HefaOS is trying to protect. Rust is a direction, not yet a public API or a commitment to a particular runtime ecosystem.

This document describes intent. The [specification](specification.md) is normative.

## Product idea

A robot application is a set of computations connected by typed data movement. Each computation owns its private state. Time, queueing, freshness, failure, and resource limits are part of the application contract rather than hidden runtime behavior.

The same conceptual model must support:

- periodic work, such as control and sampling;
- data-driven work, such as reacting to a new observation;
- goal-driven work, such as carrying an admitted motion objective to completion.

Applications define the timing they need. HefaOS does not impose a universal frequency or force a system-wide choice such as 20 Hz, 200 Hz, or any fixed range. Feasibility and admission must be evaluated against the complete set of declared temporal and resource contracts.

## Local autonomy first

The local control path must remain useful without AI inference, network connectivity, storage services, fleet coordination, or cloud control. A classical-control-only robot is a complete product configuration.

Learned or hybrid behavior can extend the system, but it cannot weaken command admission, independent protection, bounded resource use, or safe local behavior.

## Meaning before mechanism

HefaOS distinguishes:

`observation -> estimate -> prediction -> proposal -> reference -> command -> outcome`

These meanings are not interchangeable even if some future implementation uses similar data layouts. In particular, a proposal is not an actuator command. Motion reaches hardware only after admission, command production, and independent protection.

## Safety and evidence

Safety-relevant uncertainty fails closed. Failures must be bounded, observable, and attributable to an owner. Claims about correctness, timing, determinism, compatibility, or safety extend only as far as preserved reproducible evidence supports them.

## What remains undecided

The concept does not select an executor backend, IPC mechanism, asynchronous runtime, simulator, model runtime, authoring language or DSL, public Rust API, or concrete type names. Those choices require architecture decisions and, where needed, a narrow evidence-producing probe.
