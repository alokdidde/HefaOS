# Evidence progress ledger

This is the live, evidence-linked status ledger. A status only changes after
the linked exit evidence is accepted; a planned document or passing local test
does not complete a delivery gate.

| Gate | Acceptance artifact | Owner | Status | Evidence |
| --- | --- | --- | --- | --- |
| 0 | Direct hand-written Copper v1.1.1 spike | HefaOS core | Partial (Experimental; raw bundle local-only) | [Frozen acceptance](gate-0-copper-spike.md); [raw evidence record](evidence/gate-0-copper-v1.1.1-8b79968.md) for clean commit `8b79968`. A durable clone-portable archive is still required. |
| 0 | Scope and frozen virtual-fixture workloads (0.2) | HefaOS core | Accepted (Experimental) | [Frozen scope/fixture lock](gate-0-scope-fixture-lock.md): SO-101 mock, pinned MuJoCo identity, exact twelve-scenario corpus, nominal static 200 Hz workload, equality guards, reference environment, and reproduction commands |
| 0 | First hardware target/profile and guarded qualification plan | HefaOS core | Open | Not part of artifact 0.2; no hardware, powered-work, or safety-protocol qualification is accepted |
| 0 | ROS comparison protocol and safety-controller target/protocol | HefaOS core | Open | Artifact 0.2 freezes the semantic corpus only; ROS protocol fields and the safety decision have explicit acceptance criteria in the scope/fixture lock |
| 1 | Hermetic frontend and HefaOS IR | HefaOS core | Blocked by prerequisite gate | Gate 0 incomplete |
| 2–10 | Later roadmap gates | HefaOS core | Blocked by prerequisite gate | Gate 0 incomplete |

## Current iteration

- Scope: Gate 0 artifacts 0.1 (direct Copper spike) and 0.2 (virtual
  scope/fixture lock) are accepted as experimental evidence; see
  [the scope/fixture lock](gate-0-scope-fixture-lock.md).
- Architecture decision: extension at the testbench `Subject` boundary; Copper
  stays outside durable bench contracts and no control IPC boundary is admitted.
- Experimental evidence: atomic Rust 1.95/Copper v1.1.1 pin, direct upstream
  reference execution, hand-written spike implementation, locally retained
  corpus/replay evidence, nominal rate characterization, bridge rejection, and
  independent final review. The raw bundle is not clone-portably archived.
- Next evidence required: the owned first-target profile and guarded
  qualification plan; the ROS comparison protocol; the safety-controller
  decision; and a durable raw-evidence archive. A ROS bridge itself remains
  explicitly excluded from Gate 0 and deferred to Gate 6.
