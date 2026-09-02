#!/usr/bin/env python3
"""Fail-closed retention maintenance for HefaOS development evidence.

This tool only considers direct run directories below configured evidence roots. It
does not discover or delete Cargo, upstream, or build caches. It is dry-run by
default; confirmed recommendation mode never mutates evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import stat
import subprocess
import sys
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCHEMA_VERSION = "hefaos.evidence-maintenance-report.v1"
CONFIRMATION = "EMIT-MANUAL-RECOVERY-RECOMMENDATIONS"


@dataclass(frozen=True)
class Candidate:
    path: Path
    state: str | None
    eligible: bool
    protection: str | None


def diagnostic(code: str, path: Path | None = None, detail: str | None = None) -> dict[str, str]:
    item = {"code": code}
    if path is not None:
        item["path"] = path.as_posix()
    if detail is not None:
        item["detail"] = detail
    return item


def load_policy(path: Path) -> dict[str, Any]:
    try:
        policy = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read policy: {error}") from error
    if not isinstance(policy, dict):
        raise ValueError("policy must be a JSON object")
    if policy.get("schema_version") != "hefaos.evidence-retention-policy.v1":
        raise ValueError("unsupported or missing policy schema_version")
    required = (
        "scratch_relative_path", "minimum_free_bytes", "evidence_roots", "cache_roots",
        "state_file", "active_marker", "maximum_state_file_bytes", "archive_receipt_ledger", "require_clean_tracked_archive_receipt_ledger", "minimum_recommendation_age_seconds", "recommendable_states", "states",
    )
    missing = [key for key in required if key not in policy]
    if missing:
        raise ValueError(f"policy is missing required keys: {', '.join(missing)}")
    if not isinstance(policy.get("policy_id"), str) or not policy["policy_id"]:
        raise ValueError("policy_id must be a nonempty string")
    for key in ("scratch_relative_path", "archive_receipt_ledger"):
        if not isinstance(policy[key], str) or not policy[key]:
            raise ValueError(f"{key} must be a nonempty string")
    for key in ("minimum_free_bytes", "maximum_state_file_bytes", "minimum_recommendation_age_seconds"):
        if isinstance(policy[key], bool) or not isinstance(policy[key], int) or policy[key] < 0:
            raise ValueError(f"{key} must be a nonnegative integer")
    if policy["maximum_state_file_bytes"] == 0:
        raise ValueError("maximum_state_file_bytes must be positive")
    for key in ("evidence_roots", "cache_roots", "recommendable_states"):
        if not isinstance(policy[key], list) or not policy[key] or not all(isinstance(value, str) and value for value in policy[key]):
            raise ValueError(f"{key} must be a nonempty array of strings")
    expected_states = ("accepted_external_verified", "review_pending", "failed", "partial")
    if not isinstance(policy["states"], dict) or tuple(policy["states"]) != expected_states or not all(isinstance(value, str) for value in policy["states"].values()):
        raise ValueError("states must be an object with string keys and values")
    if policy["recommendable_states"] != ["failed", "partial"]:
        raise ValueError("recommendable_states must be exactly [failed, partial]")
    if not isinstance(policy["require_clean_tracked_archive_receipt_ledger"], bool):
        raise ValueError("require_clean_tracked_archive_receipt_ledger must be boolean")
    for key in ("state_file", "active_marker"):
        if not isinstance(policy[key], str) or not policy[key]:
            raise ValueError(f"{key} must be a nonempty string")
        name = Path(policy[key])
        if name.is_absolute() or len(name.parts) != 1 or name.name in ("", ".", ".."):
            raise ValueError(f"{key} must be a single filename")
    return policy


def canonical_tree_digest(candidate: Path, policy: dict[str, Any]) -> str:
    """Return the v1 collision-free manifest digest for immutable evidence files."""
    digest = hashlib.sha256()
    digest.update(b"hefaos.evidence-tree-manifest.v1\0")
    excluded_at_root = {policy["state_file"], policy["active_marker"]}
    entries = sorted(candidate.rglob("*"), key=lambda path: path.relative_to(candidate).as_posix())
    for entry in entries:
        relative = entry.relative_to(candidate).as_posix()
        try:
            entry_stat = entry.lstat()
        except OSError as error:
            raise ValueError(f"cannot stat evidence tree entry {relative}: {error}") from error
        if entry.is_symlink() or not (stat.S_ISDIR(entry_stat.st_mode) or stat.S_ISREG(entry_stat.st_mode)):
            raise ValueError(f"unsafe evidence tree entry: {relative}")
        if entry.parent == candidate and entry.name in excluded_at_root:
            continue
        if stat.S_ISDIR(entry_stat.st_mode):
            continue
        path_bytes = relative.encode("utf-8")
        file_digest = hashlib.sha256()
        with entry.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                file_digest.update(chunk)
        digest.update(b"F")
        digest.update(len(path_bytes).to_bytes(8, "big"))
        digest.update(path_bytes)
        digest.update(entry_stat.st_size.to_bytes(8, "big"))
        digest.update(file_digest.digest())
    return digest.hexdigest()


def load_receipts(root: Path, policy: dict[str, Any]) -> dict[str, dict[str, str]]:
    ledger_path = relative_path(root, policy["archive_receipt_ledger"], "archive_receipt_ledger")
    try:
        ledger_stat = ledger_path.lstat()
        if ledger_path.is_symlink() or not stat.S_ISREG(ledger_stat.st_mode) or ledger_stat.st_size > policy["maximum_state_file_bytes"]:
            raise ValueError("invalid receipt ledger file")
        ledger = json.loads(ledger_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read archive receipt ledger: {error}") from error
    if not isinstance(ledger, dict) or ledger.get("schema_version") != "hefaos.evidence-archive-receipts.v1":
        raise ValueError("unsupported archive receipt ledger")
    entries = ledger.get("receipts")
    if not isinstance(entries, list):
        raise ValueError("archive receipt ledger receipts must be an array")
    receipts: dict[str, dict[str, str]] = {}
    for entry in entries:
        if not isinstance(entry, dict):
            raise ValueError("archive receipt ledger entry must be an object")
        required = ("receipt_id", "evidence_tree_sha256", "immutable_uri", "verified_at", "verification_manifest_sha256")
        if not all(isinstance(entry.get(key), str) and entry[key] for key in required):
            raise ValueError("archive receipt ledger entry is incomplete")
        receipt_id = entry["receipt_id"]
        if receipt_id in receipts:
            raise ValueError("archive receipt ledger contains duplicate receipt_id")
        receipts[receipt_id] = {key: entry[key] for key in required}
    return receipts


def require_clean_tracked_receipt_ledger(root: Path, policy: dict[str, Any]) -> None:
    if not policy["require_clean_tracked_archive_receipt_ledger"]:
        return
    ledger = policy["archive_receipt_ledger"]
    commands = (
        ("git", "-C", str(root), "ls-files", "--error-unmatch", "--", ledger),
        ("git", "-C", str(root), "diff", "--quiet", "--", ledger),
        ("git", "-C", str(root), "diff", "--cached", "--quiet", "--", ledger),
    )
    for command in commands:
        if subprocess.run(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False).returncode != 0:
            raise ValueError("EVIDENCE_RECEIPT_LEDGER_NOT_CLEAN_TRACKED")


def within(root: Path, child: Path) -> bool:
    try:
        child.resolve(strict=False).relative_to(root.resolve(strict=False))
    except ValueError:
        return False
    return True


def relative_path(root: Path, value: str, label: str) -> Path:
    path = Path(value)
    if path.is_absolute() or ".." in path.parts:
        raise ValueError(f"{label} must be a repository-relative path without '..'")
    resolved = root / path
    if not within(root, resolved):
        raise ValueError(f"{label} resolves outside the repository")
    return resolved


def validate_scratch(root: Path, scratch: Path, policy: dict[str, Any]) -> None:
    relative = scratch.relative_to(root)
    current = root
    for part in relative.parts:
        current = current / part
        if current.is_symlink():
            raise ValueError("scratch_relative_path must not traverse a symlink")
    if scratch.exists() and not scratch.is_dir():
        raise ValueError("scratch_relative_path must name a directory")
    caches = [relative_path(root, value, "cache_roots entry") for value in policy["cache_roots"]]
    evidence = [relative_path(root, value, "evidence_roots entry") for value in policy["evidence_roots"]]
    if not any(within(cache, scratch) for cache in caches):
        raise ValueError("scratch_relative_path must be contained by a declared cache root")
    if any(within(evidence_root, scratch) or within(scratch, evidence_root) for evidence_root in evidence):
        raise ValueError("scratch_relative_path must be disjoint from evidence roots")


def inspect_candidate(candidate: Path, evidence_root: Path, policy: dict[str, Any], receipts: dict[str, dict[str, str]], now: float) -> Candidate:
    if candidate.is_symlink():
        return Candidate(candidate, None, False, "symlink")
    if not within(evidence_root, candidate):
        return Candidate(candidate, None, False, "out_of_root")
    if not candidate.is_dir():
        return Candidate(candidate, None, False, "not_directory")
    if os.path.lexists(candidate / policy["active_marker"]):
        return Candidate(candidate, None, False, "active")

    state_path = candidate / policy["state_file"]
    try:
        state_stat = state_path.lstat()
    except OSError:
        return Candidate(candidate, None, False, "unknown")
    if state_path.is_symlink():
        return Candidate(candidate, None, False, "state_symlink")
    if not stat.S_ISREG(state_stat.st_mode) or state_stat.st_size > policy["maximum_state_file_bytes"]:
        return Candidate(candidate, None, False, "unknown")
    try:
        record = json.loads(state_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return Candidate(candidate, None, False, "unknown")
    if not isinstance(record, dict):
        return Candidate(candidate, None, False, "unknown")
    if record.get("schema_version") != "hefaos.evidence-state.v1":
        return Candidate(candidate, None, False, "unknown")

    state = record.get("state")
    if not isinstance(state, str) or state not in policy["states"]:
        return Candidate(candidate, None, False, "unknown")
    if state == "accepted_external_verified":
        return Candidate(candidate, state, False, "accepted")
    if state == "review_pending":
        return Candidate(candidate, state, False, "review_pending")
    if record.get("archive_scope") == "local_only":
        return Candidate(candidate, state, False, "local_only")
    receipt_id = record.get("archive_receipt_id")
    digest = record.get("evidence_tree_sha256")
    receipt = receipts.get(receipt_id) if isinstance(receipt_id, str) else None
    if record.get("archive_scope") != "external_verified" or not isinstance(digest, str) or receipt is None:
        return Candidate(candidate, state, False, "archive_receipt_missing")
    if receipt["evidence_tree_sha256"] != digest:
        return Candidate(candidate, state, False, "archive_receipt_missing")
    try:
        if canonical_tree_digest(candidate, policy) != digest:
            return Candidate(candidate, state, False, "tree_digest_mismatch")
    except ValueError:
        return Candidate(candidate, state, False, "tree_digest_mismatch")
    if record.get("manual_recovery_eligible") is not True:
        return Candidate(candidate, state, False, "not_manual_recovery_eligible")
    age = now - candidate.stat().st_mtime
    if age < policy["minimum_recommendation_age_seconds"]:
        return Candidate(candidate, state, False, "grace_period")
    if state not in policy["recommendable_states"]:
        return Candidate(candidate, state, False, "state_protected")
    return Candidate(candidate, state, True, None)


def discover(root: Path, policy: dict[str, Any], receipts: dict[str, dict[str, str]], now: float) -> list[Candidate]:
    candidates: list[Candidate] = []
    receipt_ledger = relative_path(root, policy["archive_receipt_ledger"], "archive_receipt_ledger")
    for root_name in sorted(policy["evidence_roots"]):
        evidence_root = relative_path(root, root_name, "evidence_roots entry")
        if evidence_root.is_symlink():
            candidates.append(Candidate(evidence_root, None, False, "symlink"))
            continue
        if not evidence_root.exists():
            continue
        if not evidence_root.is_dir() or not within(root, evidence_root):
            candidates.append(Candidate(evidence_root, None, False, "out_of_root"))
            continue
        for gate in sorted(evidence_root.iterdir(), key=lambda path: path.name):
            if gate == receipt_ledger:
                continue
            if gate.is_symlink() or not gate.is_dir():
                candidates.append(inspect_candidate(gate, evidence_root, policy, receipts, now))
                continue
            for run in sorted(gate.iterdir(), key=lambda path: path.name):
                candidates.append(inspect_candidate(run, evidence_root, policy, receipts, now))
    return candidates


def report_for(root: Path, policy: dict[str, Any], mode: str, candidates: list[Candidate]) -> dict[str, Any]:
    counters: dict[str, int] = {
        "accepted_protected": 0,
        "active_protected": 0,
        "manual_recovery_candidates": 0,
        "cache_roots_excluded": 0,
        "failed": 0,
        "partial": 0,
        "review_pending_protected": 0,
        "symlink_or_out_of_root_protected": 0,
        "unknown_protected": 0,
    }
    diagnostics: list[dict[str, str]] = []
    for cache_name in sorted(policy["cache_roots"]):
        cache_path = relative_path(root, cache_name, "cache_roots entry")
        if cache_path.exists() or cache_path.is_symlink():
            counters["cache_roots_excluded"] += 1
            diagnostics.append(diagnostic("EVIDENCE_CACHE_ROOT_EXCLUDED", cache_path))
    for candidate in candidates:
        if candidate.state in ("failed", "partial"):
            counters[candidate.state] += 1
        if candidate.eligible:
            counters["manual_recovery_candidates"] += 1
            diagnostics.append(diagnostic("EVIDENCE_MANUAL_RECOVERY_CANDIDATE", candidate.path, candidate.state))
            continue
        code = {
            "accepted": ("accepted_protected", "EVIDENCE_ACCEPTED_PROTECTED"),
            "active": ("active_protected", "EVIDENCE_ACTIVE_PROTECTED"),
            "review_pending": ("review_pending_protected", "EVIDENCE_REVIEW_PENDING_PROTECTED"),
            "symlink": ("symlink_or_out_of_root_protected", "EVIDENCE_SYMLINK_PROTECTED"),
            "state_symlink": ("symlink_or_out_of_root_protected", "EVIDENCE_SYMLINK_PROTECTED"),
            "out_of_root": ("symlink_or_out_of_root_protected", "EVIDENCE_OUT_OF_ROOT_PROTECTED"),
        }.get(candidate.protection or "", ("unknown_protected", "EVIDENCE_UNKNOWN_PROTECTED"))
        counters[code[0]] += 1
        diagnostics.append(diagnostic(code[1], candidate.path, candidate.protection))
    for item in diagnostics:
        if "path" in item:
            path = Path(item["path"])
            if path.is_absolute():
                item["path"] = path.relative_to(root).as_posix()
    return {
        "schema_version": SCHEMA_VERSION,
        "mode": mode,
        "policy_id": policy["policy_id"],
        "policy_schema_version": policy["schema_version"],
        "counters": counters,
        "diagnostics": sorted(diagnostics, key=lambda item: (item["code"], item.get("path", ""))),
    }


def open_scratch_dir(root: Path, scratch: Path) -> int:
    descriptor = os.open(root, os.O_RDONLY | os.O_DIRECTORY)
    try:
        for part in scratch.relative_to(root).parts:
            try:
                os.mkdir(part, dir_fd=descriptor)
            except FileExistsError:
                pass
            next_descriptor = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=descriptor)
            os.close(descriptor)
            descriptor = next_descriptor
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def ensure_space(scratch_fd: int, minimum: int) -> None:
    available = os.fstatvfs(scratch_fd).f_bavail * os.fstatvfs(scratch_fd).f_frsize
    if available < minimum:
        raise RuntimeError(f"EVIDENCE_INSUFFICIENT_SPACE available={available} required={minimum}")


def ensure_space_before_create(root: Path, scratch: Path, minimum: int) -> None:
    descriptor = os.open(root, os.O_RDONLY | os.O_DIRECTORY)
    try:
        for part in scratch.relative_to(root).parts:
            try:
                next_descriptor = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=descriptor)
            except FileNotFoundError:
                break
            os.close(descriptor)
            descriptor = next_descriptor
        ensure_space(descriptor, minimum)
    finally:
        os.close(descriptor)


def write_report(scratch_fd: int, report: dict[str, Any]) -> None:
    report_name = "last-maintenance-report-v1.json"
    temporary = f".{report_name}.{uuid.uuid4().hex}.tmp"
    try:
        descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600, dir_fd=scratch_fd)
        try:
            payload = (json.dumps(report, indent=2, sort_keys=True) + "\n").encode("utf-8")
            remaining = memoryview(payload)
            while remaining:
                written = os.write(descriptor, remaining)
                if written <= 0:
                    raise OSError("EVIDENCE_REPORT_SHORT_WRITE")
                remaining = remaining[written:]
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        os.replace(temporary, report_name, src_dir_fd=scratch_fd, dst_dir_fd=scratch_fd)
        os.fsync(scratch_fd)
    except BaseException:
        try:
            os.unlink(temporary, dir_fd=scratch_fd)
        except FileNotFoundError:
            pass
        raise


def normalize_report_paths(root: Path, report: dict[str, Any]) -> None:
    for item in report["diagnostics"]:
        if "path" in item:
            path = Path(item["path"])
            if path.is_absolute():
                item["path"] = path.relative_to(root).as_posix()
    report["diagnostics"].sort(key=lambda item: (item["code"], item.get("path", "")))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd(), help="repository root")
    parser.add_argument("--policy", type=Path, default=Path("tools/evidence/retention-policy-v1.json"))
    parser.add_argument("--recommend", action="store_true", help="emit confirmed manual-recovery recommendations; never deletes evidence")
    parser.add_argument("--confirm-recommendation", help=f"required exact phrase: {CONFIRMATION}")
    arguments = parser.parse_args(argv)
    root = arguments.root.resolve()
    policy_path = arguments.policy if arguments.policy.is_absolute() else root / arguments.policy
    scratch_fd: int | None = None
    try:
        policy = load_policy(policy_path)
        scratch = relative_path(root, policy["scratch_relative_path"], "scratch_relative_path")
        validate_scratch(root, scratch, policy)
        receipts = load_receipts(root, policy)
        if arguments.recommend and arguments.confirm_recommendation != CONFIRMATION:
            raise ValueError("EVIDENCE_CONFIRMATION_REQUIRED")
        if arguments.recommend:
            require_clean_tracked_receipt_ledger(root, policy)
        ensure_space_before_create(root, scratch, int(policy["minimum_free_bytes"]))
        scratch_fd = open_scratch_dir(root, scratch)
        ensure_space(scratch_fd, int(policy["minimum_free_bytes"]))
        candidates = discover(root, policy, receipts, time.time())
        mode = "recommendation" if arguments.recommend else "dry_run"
        report = report_for(root, policy, mode, candidates)
        if arguments.recommend:
            report["phase"] = "pre_recommendation"
            write_report(scratch_fd, report)
        if arguments.recommend:
            for candidate in candidates:
                if candidate.eligible:
                    refreshed = inspect_candidate(candidate.path, candidate.path.parents[1], policy, receipts, time.time())
                    if refreshed.eligible:
                        report["diagnostics"].append(diagnostic("EVIDENCE_MANUAL_RECOVERY_RECOMMENDED", candidate.path, candidate.state))
                    else:
                        report["diagnostics"].append(diagnostic("EVIDENCE_FULL_REVIEW_REQUIRED", candidate.path, refreshed.protection))
        report["phase"] = "complete"
        normalize_report_paths(root, report)
        write_report(scratch_fd, report)
        print(json.dumps(report, indent=2, sort_keys=True))
        return 0
    except (ValueError, RuntimeError, OSError) as error:
        print(str(error), file=sys.stderr)
        return 2
    finally:
        if scratch_fd is not None:
            try:
                os.close(scratch_fd)
            except OSError:
                pass


if __name__ == "__main__":
    raise SystemExit(main())
