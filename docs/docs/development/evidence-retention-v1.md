# Development evidence retention policy v1

**Status:** Implemented development tooling; it makes no archival, acceptance,
qualification, performance, or safety claim.

This policy governs space maintenance for raw development evidence. It is an
extension of the development-evidence ledger boundary, not of the runtime,
control, replay, IR, or safety planes. The versioned machine policy is
[`tools/evidence/retention-policy-v1.json`](../../../tools/evidence/retention-policy-v1.json).

## Frozen acceptance criteria

The maintenance tool only considers direct run directories under the configured
`evidence/` root. It never treats `target/`, `testbench/.cache/`, `runtime/build/`,
or `build/` as archive candidates: these are separately reported build/upstream/Cargo
cache roots. A run is eligible only if a non-symlink
`.hefaos-evidence-state.json` has schema `hefaos.evidence-state.v1`, declares
`archive_scope: "external_verified"`, a structured external archive receipt
ID and evidence digest matching the tracked
[`evidence/archive-receipts-v1.json`](../../../evidence/archive-receipts-v1.json)
ledger, `manual_recovery_eligible: true`, and has state `failed` or `partial` after the
configured 30-day grace period. Recommendation mode rejects an untracked or modified
receipt ledger. A self-authored receipt in a run directory is not authorization.
`local_only` is always protected.

Eligibility recomputes `hefaos.evidence-tree-manifest.v1`: sorted complete
root-relative regular-file paths, each framed with a file marker, eight-byte path
length, UTF-8 path, eight-byte byte length, and the file's SHA-256. Only the
root-level mutable state and active-marker files are excluded; identically named
nested files are evidence. Symlinks, special files, or a digest mismatch are
protected.

The state tiers are fixed in v1:

- `accepted_external_verified`: retained. This tier requires an external immutable
  archive receipt outside this tool; the tool cannot grant it.
- `review_pending`: retained.
- `failed` and `partial`: always retained. When every eligibility guard succeeds,
  the tool may emit a manual-recovery recommendation; it never removes the bundle.

The tool protects accepted, review-pending, active-marker, unknown/malformed,
symlink, and out-of-root entries. Missing state is unknown, never a manual-recovery
candidate. There is deliberately no state that upgrades an existing bundle from
local-only to externally verified. Existing bundles are unmodified and remain
unknown/protected until a reviewer creates a valid state record.

## Operation, workload, and environment

Use a checkout with Python 3.11+ and no runtime process interaction:

```bash
python3 tools/evidence/maintain_evidence.py
python3 tools/evidence/maintain_evidence.py --recommend \
  --confirm-recommendation EMIT-MANUAL-RECOVERY-RECOMMENDATIONS
```

The first command is the required dry-run workload. It writes the deterministic
report to ignored `target/evidence-maintenance/last-maintenance-report-v1.json`.
`--recommend` still requires both flags and the exact confirmation phrase, but is a
confirmed recommendation mode: it never mutates evidence and emits
`EVIDENCE_MANUAL_RECOVERY_RECOMMENDED` only after it reinspects that bundle.
If it changed after the initial scan, it emits `EVIDENCE_FULL_REVIEW_REQUIRED`
instead. Neither mode mutates evidence.
This is deliberate. On ordinary Linux filesystems, process locks are advisory and
open descriptors survive rename, so the tool cannot guarantee that a rogue or
already-open producer will not mutate a tree during a hypothetical recursive
removal. Before
creating scratch output, the tool checks the configured 64 MiB free-space threshold
and exits nonzero without mutation when space is insufficient.

## Correctness guards and measurable thresholds

The stable report schema is `hefaos.evidence-maintenance-report.v1`. Counters are:
`manual_recovery_candidates`, `accepted_protected`, `review_pending_protected`,
`active_protected`, `unknown_protected`, `symlink_or_out_of_root_protected`,
`failed`, `partial`, and `cache_roots_excluded`. Stable diagnostic codes make the
protection decision auditable, including `EVIDENCE_CACHE_ROOT_EXCLUDED`,
`EVIDENCE_MANUAL_RECOVERY_CANDIDATE`, `EVIDENCE_MANUAL_RECOVERY_RECOMMENDED`, and
`EVIDENCE_INSUFFICIENT_SPACE`.

The focused reproduction and correctness check is:

```bash
python3 -m unittest tools/evidence/tests/test_maintain_evidence.py
python3 tools/evidence/maintain_evidence.py
```

The tests use a checked-in synthetic failed/externally-archived state-record fixture and prove dry-run
non-mutation, explicit-confirmation gating, state-tier and active/unknown/symlink
protection, cache exclusion, fail-closed space handling, receipt-ledger binding,
collision-free manifest framing, nested metadata coverage, and active-marker
mutation after eligibility scanning.
They do not archive a bundle or change any ledger status.

## External archive blocker

The tracked receipt ledger is intentionally empty. The existing Gate 0 raw bundles
remain local-only. A durable immutable, clone-portable archive location and an
independently reviewed ledger receipt binding the existing raw contents are still
required before any record can use `accepted_external_verified`. This policy
intentionally does not create a receipt or alter an accepted-evidence status.
