# Evidence progress ledger

This is the live, evidence-linked status ledger. A status only changes after
the linked exit evidence is accepted; a planned document or passing local test
does not complete a delivery gate.

| Gate | Acceptance artifact | Owner | Status | Evidence |
| --- | --- | --- | --- | --- |
| 0 | Direct hand-written Copper v1.1.1 spike | HefaOS core | In progress | [Frozen acceptance](gate-0-copper-spike.md); baseline workspace checks passed locally on 2026-09-01; Copper v1.1.1 under Rust 1.93.0 failed as expected because its declared MSRV is 1.95.0 |
| 0 | Scope, target profile, and fixed comparison workloads | HefaOS core | Partial | SO-101 v0 scenario corpus and pinned MuJoCo model exist; hardware and ROS comparison artifacts remain open |
| 1 | Hermetic frontend and HefaOS IR | HefaOS core | Blocked by prerequisite gate | Gate 0 incomplete |
| 2–10 | Later roadmap gates | HefaOS core | Blocked by prerequisite gate | Gate 0 incomplete |

## Current iteration

- Scope: Gate 0 artifact 0.1 only, as frozen in
  [the acceptance document](gate-0-copper-spike.md).
- Architecture decision: extension at the testbench `Subject` boundary; Copper
  stays outside durable bench contracts and no control IPC boundary is admitted.
- Next evidence required: atomic Rust 1.95/Copper v1.1.1 pin, direct upstream
  reference execution, hand-written spike implementation, raw run/replay
  evidence, and independent final review.
