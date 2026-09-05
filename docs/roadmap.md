# HefaOS greenfield roadmap

The roadmap controls sequence and completion status. Work proceeds from architecture to a narrow evidence-producing probe and only then to implementation. No later item may begin while an earlier required item is incomplete.

## Artifact protocol

Before every artifact, record and freeze:

- acceptance criteria;
- workload or documentary question;
- environment and pinned inputs;
- correctness, safety, and security guards;
- metrics and immutable thresholds;
- clean-checkout reproduction command;
- expected raw evidence and storage location.

Use one author and a separate fresh-context critic. The critic reviews the actual final diff, checks, reference output, benchmarks, and evidence that apply. Resolve all actionable findings and rerun every frozen check on the final tree. An artifact passes only when its linked evidence passes; a narrative status is insufficient.

## Reset 0 — documentation-only baseline

**Status: in progress.** It is complete only after both artifacts pass on the final tree.

### Reset 0.1 — canonical documents

**Status: passed.** Final-tree verification is repeated after Reset 0.2.

Frozen workload and environment: inspect the canonical files and the preserved proposal from a clean checkout using Git, `sha256sum`, and Python 3; no build toolchain or network access is permitted or required.

Acceptance:

- the redesign proposal is preserved byte-for-byte with its recorded digest;
- concept, architecture, specification, reset ADR, roadmap, and bootstrap documents are concise and internally consistent;
- document precedence and proposal non-authority are explicit;
- no document claims that deleted code is implemented, measured, qualified, or supported;
- unresolved technology and API choices remain unresolved;
- all relative Markdown links resolve.

Correctness guards: the proposal is never rewritten, proposals never gain normative authority, implementation status is never inferred, and unresolved backend, IPC, async-runtime, simulator, model-runtime, authoring, API, and concrete-type choices remain open.

Metrics and thresholds: proposal digest mismatches = 0; missing relative-link targets = 0; actionable critic findings = 0; unsupported present-tense implementation or evidence claims = 0.

### Reset 0.2 — active-tree reset

**Status: in progress.**

Frozen workload and environment: inspect the complete tracked-file inventory and recovery refs from a clean checkout using Git and standard POSIX text tools; no build or network access is permitted or required.

Acceptance:

- the active tree contains only the root governance files and canonical documentation listed in the reset ADR;
- no product implementation, testbench, examples, tools, models, dependency manifests, generated files, implementation-specific CI, or legacy specification remains tracked;
- the recovery commit, branch, and tag recorded in the reset ADR resolve;
- generated binaries, models, logs, crashes, local evidence, dependencies, and build output are ignored;
- the final diff passes whitespace validation.

Correctness guards: the proposal and recovery refs remain intact; only the allowlisted active files remain tracked; generated and local artifacts remain untracked; no deleted implementation is copied into the new baseline.

Metrics and thresholds: tracked paths outside the allowlist = 0; missing allowlisted paths = 0; unresolved recovery refs = 0; ignored-artifact leaks = 0; whitespace errors = 0; actionable critic findings = 0.

Clean-checkout reproduction commands:

```bash
git diff --check pre-greenfield-v2-reset-2026-09-05 HEAD
printf '%s  %s\n' '5dff93305ce355a72a8b9f62f5ea37477ddffe4c2b1915a85faaedc577e1e81f' 'docs/proposals/hefaos-redesign-2026-09-05.md' | sha256sum --check -
git rev-parse --verify bde7adf^{commit}
git rev-parse --verify codex/archive/pre-greenfield-2026-09-05^{commit}
git rev-parse --verify pre-greenfield-v2-reset-2026-09-05^{tag}
git ls-files | LC_ALL=C sort
```

The last command's output MUST equal the active-tree allowlist in the reset ADR. Check relative Markdown targets with:

```bash
python3 - <<'PY'
from pathlib import Path
import re

files = [Path(path) for path in (
    "README.md",
    "CONTRIBUTING.md",
    "AGENTS.md",
    "docs/concept.md",
    "docs/architecture.md",
    "docs/specification.md",
    "docs/roadmap.md",
    "docs/decisions/0001-greenfield-rust-reset.md",
    "docs/proposals/hefaos-redesign-2026-09-05.md",
    "docs/bootstrap.md",
)]
missing = []
for document in files:
    body = document.read_text(encoding="utf-8")
    for target in re.findall(r"(?<!!)\[[^\]\n]+\]\(([^)\n]+)\)", body):
        target = target.strip().split()[0]
        if target.startswith(("http://", "https://", "mailto:", "#")):
            continue
        path = target.split("#", 1)[0]
        if path and not (document.parent / path).resolve().exists():
            missing.append(f"{document}: {target}")
if missing:
    raise SystemExit("Missing relative links:\n" + "\n".join(missing))
print("relative-links: ok")
PY
```

## A — architecture decisions

**Status: not started.** Complete the following smallest decision artifacts in order.

### A1 — first operational profile

Choose the first robot or simulator-backed use case, deployment topology, control responsibilities, hazards, unavailable-dependency behavior, application- and control-runtime stall and crash cases, and explicit non-goals. This sets the failure model and environment against which all later contracts are judged.

### A2 — authoring and stable contract boundary

Decide the Rust ownership boundary, external authoring surface if any, versioned intermediate contract, validation stage, compatibility rules, and invalid-input behavior. Do not design a broad SDK.

### A3 — execution islands and temporal admission

Define island boundaries, scheduling and delivery ownership, application-defined periods or triggers, applicable clock domains, phase, deadline, sampling, freshness, queueing, overflow, resource admission, overload, and fallback. For every cross-domain time mapping, define identity and version, uncertainty or error bound, reset and reconnect semantics, and the fail-closed result when freshness, ordering, or deadline satisfaction cannot be decided within bounds. Define how candidate execution backends will be evaluated without selecting one by intuition.

### A4 — motion authority and failure semantics

Define proposal, admission, reference, command, and protection responsibilities; identity and authority; epochs; cancellation; restart; stale data; partial physical progress; and fail-closed outcomes. Define one hardware actuation boundary for every motion-causing path, including manual and recovery paths, and demonstrate how independent protection remains effective for the A1 runtime-failure model without prematurely selecting its hardware implementation.

### A5 — evidence, compatibility, and claims

Define operational telemetry versus replay evidence, ordering and provenance requirements, version capture, raw-evidence retention, compatibility claim scope, and the form of reproducible acceptance artifacts.

### A6 — architecture readiness review

A fresh-context critic must verify every item in the specification's architecture-readiness list, every cross-document invariant, and every open choice. The review must name the highest-risk remaining uncertainty and freeze the first probe that will answer only that uncertainty.

## B — first narrow conformance probe

**Status: blocked by A.** Build the smallest throwaway or retainable Rust probe that can answer the single uncertainty selected by A6. Pin all external revisions. Preserve raw reference and subject output. Do not create a general runtime, SDK, or complete testbench around it.

The probe passes only if its frozen behavioral and resource guards pass and the result is reflected in an ADR. A failed probe is valid evidence and must not be hidden by changing the question or threshold.

## C — first vertical slice

**Status: blocked by B.** Implement only the minimum supported path for the A1 profile: typed input, private state, declared timing, admission, command production, independent protection, outcome, and scoped evidence. Grow stable interfaces only from demonstrated need.

## D — conformance system

**Status: blocked by C.** Grow reusable tests and evidence tooling one contract at a time from the accepted vertical slice. A complete testbench becomes justified only after enough stable contracts exist to define what completeness means.

## E — optional extensions

**Status: blocked by D.** Add learned or hybrid behavior, richer authoring, networking, storage, fleet coordination, and hardware profiles as isolated extensions. Each extension must prove that it cannot weaken or become a liveness dependency of the admitted local control path.
