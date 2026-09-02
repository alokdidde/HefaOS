# Evidence progress ledger

This is the live, evidence-linked status ledger. A status only changes after
the linked exit evidence is accepted; a planned document or passing local test
does not complete a delivery gate.

| Gate | Acceptance artifact | Owner | Status | Evidence |
| --- | --- | --- | --- | --- |
| 0 | Direct hand-written Copper v1.1.1 spike | HefaOS core | Accepted (Experimental) | [Frozen acceptance](gate-0-copper-spike.md); [raw evidence record](evidence/gate-0-copper-v1.1.1-8b79968.md) for clean commit `8b79968`, reviewed against the frozen bar |
| 0 | Scope, target profile, and fixed comparison workloads | HefaOS core | Partial | SO-101 v0 scenario corpus and pinned MuJoCo model exist; hardware and ROS comparison artifacts remain open |
| 1 | Hermetic frontend and HefaOS IR | HefaOS core | Blocked by prerequisite gate | Gate 0 incomplete |
| 2–10 | Later roadmap gates | HefaOS core | Blocked by prerequisite gate | Gate 0 incomplete |

## Current iteration

- Scope: Gate 0 artifact 0.1 was accepted as frozen in
  [the acceptance document](gate-0-copper-spike.md).
- Architecture decision: extension at the testbench `Subject` boundary; Copper
  stays outside durable bench contracts and no control IPC boundary is admitted.
- Accepted evidence: atomic Rust 1.95/Copper v1.1.1 pin, direct upstream
  reference execution, hand-written spike implementation, raw corpus/replay
  evidence, nominal rate characterization, bridge rejection, and independent
  final review.
- Next evidence required: Gate 0 scope, target profile, ROS comparison, and
  open-decision ownership acceptance artifact.
