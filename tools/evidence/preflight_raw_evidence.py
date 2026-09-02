#!/usr/bin/env python3
"""Fail-closed disk-budget admission for retained raw evidence bundles.

The preflight deliberately accounts allocated blocks (rather than apparent file
length) and never creates an evidence destination until its admission decision
has been made.  It is kept separate from retention maintenance: no evidence is
discarded to make a new Gate 0 run fit.
"""

from __future__ import annotations

import argparse
import ctypes
import errno
import hashlib
import json
import os
import stat
import sys
import uuid
from pathlib import Path
from typing import Any


SCHEMA_VERSION = "hefaos.raw-evidence-budget-preflight.v1"
POSTFLIGHT_SCHEMA_VERSION = "hefaos.raw-evidence-budget-postflight.v1"
PROFILE_ID = "gate-0-copper-raw-v1"
FROZEN_ROOTS = ["evidence/gate-0-copper", "target/gate-0-copper-evidence"]
FROZEN_MAXIMUM = 4294967296
FROZEN_RESERVE = 3221225472
FROZEN_FREE_FLOOR = 1073741824


class BudgetError(Exception):
    def __init__(self, code: str):
        self.code = code
        super().__init__(code)


def canonical_json(value: dict[str, Any]) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def report(
    *, schema_version: str, profile: str, policy_sha256: str | None,
    roots: list[str], current: int, maximum: int, reserve: int,
    free_floor: int, available: int, diagnostics: list[str], verdict: str,
) -> dict[str, Any]:
    return {
        "schema_version": schema_version,
        "profile": profile,
        "policy_sha256": policy_sha256,
        "accounting_roots": roots,
        "current_bytes": current,
        "maximum_bytes": maximum,
        "reserve_bytes": reserve,
        "free_floor_bytes": free_floor,
        "available_bytes": available,
        "diagnostics": [{"code": code} for code in sorted(set(diagnostics))],
        "verdict": verdict,
    }


def emit(value: dict[str, Any]) -> None:
    print(canonical_json(value), end="")


def load_policy_bytes(path: Path) -> tuple[bytes, str]:
    payload = path.read_bytes()
    return payload, hashlib.sha256(payload).hexdigest()


def is_relative_below(parent: Path, child: Path) -> bool:
    try:
        child.relative_to(parent)
    except ValueError:
        return False
    return child != parent


def safe_relative(root: Path, value: str, code: str) -> Path:
    path = Path(value)
    if path.is_absolute() or ".." in path.parts or not path.parts:
        raise BudgetError(code)
    resolved = root / path
    if not is_relative_below(root, resolved):
        raise BudgetError(code)
    return resolved


def lstat_no_symlink(path: Path, code: str) -> os.stat_result:
    try:
        entry = path.lstat()
    except OSError as error:
        raise BudgetError(code) from error
    if stat.S_ISLNK(entry.st_mode):
        raise BudgetError(code)
    return entry


def validate_existing_ancestors(root: Path, path: Path, code: str) -> None:
    """Reject a symlink, special node, or mount before `path`.

    Missing trailing components are intentional: admission creates those only
    after the budget decision.  Existing ancestors must be ordinary directories
    on the repository filesystem.
    """
    root_stat = lstat_no_symlink(root, code)
    if not stat.S_ISDIR(root_stat.st_mode):
        raise BudgetError(code)
    current = root
    for part in path.relative_to(root).parts:
        candidate = current / part
        try:
            entry = candidate.lstat()
        except FileNotFoundError:
            return
        except OSError as error:
            raise BudgetError(code) from error
        if stat.S_ISLNK(entry.st_mode) or not stat.S_ISDIR(entry.st_mode) or entry.st_dev != root_stat.st_dev:
            raise BudgetError(code)
        current = candidate


def profile_from_policy(policy: dict[str, Any], requested: str) -> dict[str, Any]:
    profiles = policy.get("raw_evidence_budget_profiles")
    if not isinstance(profiles, dict):
        raise BudgetError("EVIDENCE_BUDGET_INVALID_POLICY")
    profile = profiles.get(requested)
    if not isinstance(profile, dict):
        raise BudgetError("EVIDENCE_BUDGET_UNKNOWN_PROFILE")
    required_ints = (
        "maximum_bytes", "reserve_bytes", "free_floor_bytes", "maximum_scan_entries",
        "maximum_scan_depth", "maximum_path_bytes",
    )
    if not isinstance(profile.get("accounting_roots"), list) or len(profile["accounting_roots"]) != 2:
        raise BudgetError("EVIDENCE_BUDGET_INVALID_POLICY")
    if not all(isinstance(item, str) and item for item in profile["accounting_roots"]):
        raise BudgetError("EVIDENCE_BUDGET_INVALID_POLICY")
    for key in required_ints:
        value = profile.get(key)
        if isinstance(value, bool) or not isinstance(value, int) or value < 0:
            raise BudgetError("EVIDENCE_BUDGET_INVALID_POLICY")
    if profile["maximum_bytes"] == 0 or profile["reserve_bytes"] == 0 or profile["free_floor_bytes"] == 0:
        raise BudgetError("EVIDENCE_BUDGET_INVALID_POLICY")
    if profile["reserve_bytes"] > profile["maximum_bytes"]:
        raise BudgetError("EVIDENCE_BUDGET_INVALID_POLICY")
    if requested == PROFILE_ID and (
        profile["accounting_roots"] != FROZEN_ROOTS
        or profile["maximum_bytes"] != FROZEN_MAXIMUM
        or profile["reserve_bytes"] != FROZEN_RESERVE
        or profile["free_floor_bytes"] != FROZEN_FREE_FLOOR
    ):
        raise BudgetError("EVIDENCE_BUDGET_FROZEN_PROFILE_MISMATCH")
    return profile


def open_directory_under_root(root: Path, path: Path, code: str) -> int:
    """Open an existing repository descendant without following any component."""
    root_fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    descriptor = root_fd
    try:
        for part in path.relative_to(root).parts:
            next_descriptor = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=descriptor)
            os.close(descriptor)
            descriptor = next_descriptor
        return descriptor
    except OSError as error:
        os.close(descriptor)
        raise BudgetError(code) from error


def scan_allocated_from_fd(root_device: int, directory_fd: int, profile: dict[str, Any]) -> int:
    """Scan a duplicate of an already anchored directory descriptor."""
    working_fd = os.dup(directory_fd)
    pending: list[tuple[int, Path, int]] = []
    try:
        total = 0
        entries = 0
        pending = [(working_fd, Path(), 0)]
        working_fd = None
        while pending:
            current_fd, relative_parent, depth = pending.pop()
            try:
                children = sorted(os.listdir(current_fd))
            except OSError as error:
                os.close(current_fd)
                raise BudgetError("EVIDENCE_BUDGET_SCAN_ERROR") from error
            try:
                for name in children:
                    entries += 1
                    if entries > profile["maximum_scan_entries"]:
                        raise BudgetError("EVIDENCE_BUDGET_SCAN_ENTRY_LIMIT")
                    relative = relative_parent / name
                    if len(relative.as_posix().encode("utf-8")) > profile["maximum_path_bytes"]:
                        raise BudgetError("EVIDENCE_BUDGET_SCAN_PATH_LIMIT")
                    try:
                        entry = os.stat(name, dir_fd=current_fd, follow_symlinks=False)
                    except OSError as error:
                        raise BudgetError("EVIDENCE_BUDGET_SCAN_ERROR") from error
                    if stat.S_ISLNK(entry.st_mode):
                        raise BudgetError("EVIDENCE_BUDGET_SYMLINK")
                    if entry.st_dev != root_device:
                        raise BudgetError("EVIDENCE_BUDGET_MOUNT")
                    if stat.S_ISREG(entry.st_mode):
                        total += entry.st_blocks * 512
                    elif stat.S_ISDIR(entry.st_mode):
                        if depth + 1 > profile["maximum_scan_depth"]:
                            raise BudgetError("EVIDENCE_BUDGET_SCAN_DEPTH_LIMIT")
                        try:
                            child_fd = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=current_fd)
                        except OSError as error:
                            raise BudgetError("EVIDENCE_BUDGET_SCAN_RACE") from error
                        opened = os.fstat(child_fd)
                        if opened.st_dev != root_device:
                            os.close(child_fd)
                            raise BudgetError("EVIDENCE_BUDGET_MOUNT")
                        if opened.st_ino != entry.st_ino or opened.st_dev != entry.st_dev:
                            os.close(child_fd)
                            raise BudgetError("EVIDENCE_BUDGET_SCAN_RACE")
                        pending.append((child_fd, relative, depth + 1))
                    else:
                        raise BudgetError("EVIDENCE_BUDGET_SPECIAL_FILE")
            finally:
                os.close(current_fd)
        return total
    finally:
        if working_fd is not None:
            os.close(working_fd)
        for pending_fd, _relative, _depth in pending:
            try:
                os.close(pending_fd)
            except OSError:
                pass


def scan_allocated(root: Path, accounting_root: Path, profile: dict[str, Any]) -> int:
    """Count only regular-file allocated bytes, failing closed on unsafe trees."""
    root_stat = lstat_no_symlink(root, "EVIDENCE_BUDGET_UNSAFE_ROOT")
    try:
        directory_fd = open_directory_under_root(root, accounting_root, "EVIDENCE_BUDGET_UNSAFE_ROOT")
    except BudgetError as error:
        # A missing accounting root has no allocation; every other failed
        # component is unsafe. Open once more only to distinguish ENOENT.
        try:
            accounting_root.lstat()
        except FileNotFoundError:
            return 0
        raise error
    try:
        if os.fstat(directory_fd).st_dev != root_stat.st_dev:
            raise BudgetError("EVIDENCE_BUDGET_MOUNT")
        return scan_allocated_from_fd(root_stat.st_dev, directory_fd, profile)
    finally:
        os.close(directory_fd)


def destination_from_argument(root: Path, value: str, managed_root: Path, *, require_new: bool) -> Path:
    supplied = Path(value)
    destination = supplied if supplied.is_absolute() else root / supplied
    # Do not use resolve() for validation: a symlink is itself a rejection.
    try:
        lexical = destination.relative_to(root)
    except ValueError as error:
        raise BudgetError("EVIDENCE_BUDGET_DESTINATION_OUTSIDE_MANAGED_ROOT") from error
    if ".." in lexical.parts or not is_relative_below(managed_root, destination):
        raise BudgetError("EVIDENCE_BUDGET_DESTINATION_OUTSIDE_MANAGED_ROOT")
    validate_existing_ancestors(root, destination.parent, "EVIDENCE_BUDGET_UNSAFE_DESTINATION")
    if require_new and os.path.lexists(destination):
        raise BudgetError("EVIDENCE_BUDGET_DESTINATION_EXISTS")
    if not require_new:
        entry = lstat_no_symlink(destination, "EVIDENCE_BUDGET_UNSAFE_DESTINATION")
        if not stat.S_ISDIR(entry.st_mode):
            raise BudgetError("EVIDENCE_BUDGET_UNSAFE_DESTINATION")
    return destination


def available_bytes(root: Path, destination: Path) -> int:
    probe = destination.parent
    while not probe.exists():
        probe = probe.parent
    validate_existing_ancestors(root, probe, "EVIDENCE_BUDGET_UNSAFE_DESTINATION")
    try:
        filesystem = os.statvfs(probe)
    except OSError as error:
        raise BudgetError("EVIDENCE_BUDGET_FREE_SPACE_UNAVAILABLE") from error
    return filesystem.f_bavail * filesystem.f_frsize


def secure_create_destination(root: Path, destination: Path) -> tuple[int, int, str, str, int, int]:
    """Create a private anchored staging directory; do not publish it yet."""
    descriptor = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        parts = destination.relative_to(root).parts
        for part in parts[:-1]:
            try:
                os.mkdir(part, 0o700, dir_fd=descriptor)
            except FileExistsError:
                pass
            next_descriptor = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=descriptor)
            os.close(descriptor)
            descriptor = next_descriptor
        final_name = parts[-1]
        temporary_name = f".gate-0-copper-admission-{uuid.uuid4().hex}.tmp"
        os.mkdir(temporary_name, 0o700, dir_fd=descriptor)
        temporary_fd = os.open(temporary_name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=descriptor)
        created = os.fstat(temporary_fd)
        return temporary_fd, descriptor, temporary_name, final_name, created.st_dev, created.st_ino
    except BaseException:
        os.close(descriptor)
        raise


def publish_destination(parent_fd: int, temporary_name: str, final_name: str, device: int, inode: int) -> None:
    """Atomically publish a populated staging directory without replacement."""
    try:
        source = os.stat(temporary_name, dir_fd=parent_fd, follow_symlinks=False)
    except OSError as error:
        raise BudgetError("EVIDENCE_BUDGET_PUBLICATION_IDENTITY_MISMATCH") from error
    if not stat.S_ISDIR(source.st_mode) or source.st_dev != device or source.st_ino != inode:
        raise BudgetError("EVIDENCE_BUDGET_PUBLICATION_IDENTITY_MISMATCH")
    try:
        libc = ctypes.CDLL(None, use_errno=True)
        renameat2 = libc.renameat2
    except (AttributeError, OSError) as error:
        raise BudgetError("EVIDENCE_BUDGET_ATOMIC_PUBLISH_UNAVAILABLE") from error
    renameat2.argtypes = (ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint)
    renameat2.restype = ctypes.c_int
    if renameat2(parent_fd, temporary_name.encode("utf-8"), parent_fd, final_name.encode("utf-8"), 1) != 0:
        failure = ctypes.get_errno()
        if failure == errno.EEXIST:
            raise BudgetError("EVIDENCE_BUDGET_DESTINATION_EXISTS")
        raise BudgetError("EVIDENCE_BUDGET_ATOMIC_PUBLISH_FAILED")
    try:
        published = os.stat(final_name, dir_fd=parent_fd, follow_symlinks=False)
    except OSError as error:
        raise BudgetError("EVIDENCE_BUDGET_PUBLICATION_IDENTITY_MISMATCH") from error
    if not stat.S_ISDIR(published.st_mode) or published.st_dev != device or published.st_ino != inode:
        raise BudgetError("EVIDENCE_BUDGET_PUBLICATION_IDENTITY_MISMATCH")
    os.fsync(parent_fd)


def remove_created_destination(bundle_fd: int, parent_fd: int, name: str, device: int, inode: int) -> None:
    """Retain a failed private staging directory; never delete a path by name.

    Publication has not happened, so this hidden directory is not an admitted
    bundle. Retaining it is the fail-closed cleanup policy: even a final-stat to
    rmdir sequence can race with a replacement, while a future maintenance pass
    can inspect the original anchored staging content without overwriting data.
    """
    _ = (bundle_fd, parent_fd, name, device, inode)


def write_new_file(directory_fd: int, name: str, payload: bytes) -> None:
    descriptor = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600, dir_fd=directory_fd)
    try:
        remaining = memoryview(payload)
        while remaining:
            count = os.write(descriptor, remaining)
            if count <= 0:
                raise OSError("EVIDENCE_BUDGET_SHORT_WRITE")
            remaining = remaining[count:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def read_regular_file(directory_fd: int, name: str, maximum_bytes: int) -> bytes:
    entry = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
    if stat.S_ISLNK(entry.st_mode) or not stat.S_ISREG(entry.st_mode) or entry.st_size > maximum_bytes:
        raise BudgetError("EVIDENCE_BUDGET_UNSAFE_PROVENANCE")
    descriptor = os.open(name, os.O_RDONLY | os.O_NOFOLLOW, dir_fd=directory_fd)
    try:
        return os.read(descriptor, maximum_bytes + 1)
    finally:
        os.close(descriptor)


def available_bytes_from_fd(directory_fd: int) -> int:
    filesystem = os.fstatvfs(directory_fd)
    return filesystem.f_bavail * filesystem.f_frsize


def padded_report_payload(value: dict[str, Any]) -> bytes:
    """Keep the mutable postflight report's allocated size stable at 8 KiB."""
    stored = dict(value)
    stored["report_padding"] = ""
    baseline = canonical_json(stored).encode("utf-8")
    padding = max(0, 8192 - len(baseline))
    stored["report_padding"] = " " * padding
    return canonical_json(stored).encode("utf-8")


def replace_file(directory_fd: int, name: str, payload: bytes) -> None:
    temporary = f".{name}.{uuid.uuid4().hex}.tmp"
    try:
        write_new_file(directory_fd, temporary, payload)
        os.replace(temporary, name, src_dir_fd=directory_fd, dst_dir_fd=directory_fd)
        os.fsync(directory_fd)
    except BaseException:
        try:
            os.unlink(temporary, dir_fd=directory_fd)
        except FileNotFoundError:
            pass
        raise


def admission(
    root: Path, profile_name: str, policy_bytes: bytes, policy_sha256: str, destination_value: str, *,
    destination_must_be_new: bool, check_budget: bool = True,
) -> tuple[dict[str, Any], Path, dict[str, Any], str]:
    try:
        policy = json.loads(policy_bytes)
    except json.JSONDecodeError as error:
        raise BudgetError("EVIDENCE_BUDGET_INVALID_POLICY") from error
    if not isinstance(policy, dict) or policy.get("schema_version") != "hefaos.evidence-retention-policy.v1":
        raise BudgetError("EVIDENCE_BUDGET_INVALID_POLICY")
    profile = profile_from_policy(policy, profile_name)
    roots = [str(item) for item in profile["accounting_roots"]]
    accounting = [safe_relative(root, item, "EVIDENCE_BUDGET_INVALID_POLICY") for item in roots]
    for item in accounting:
        # `Path.lstat()` on the terminal path alone follows any symlink in an
        # ancestor. Validate the whole existing component chain before scan.
        validate_existing_ancestors(root, item, "EVIDENCE_BUDGET_UNSAFE_ROOT")
    managed_root = accounting[1]
    destination = destination_from_argument(root, destination_value, managed_root, require_new=destination_must_be_new)
    current = sum(scan_allocated(root, item, profile) for item in accounting)
    available = available_bytes(root, destination)
    diagnostics: list[str] = []
    if check_budget and current + profile["reserve_bytes"] > profile["maximum_bytes"]:
        diagnostics.append("EVIDENCE_BUDGET_MAXIMUM_EXCEEDED")
    if check_budget and available < profile["reserve_bytes"] + profile["free_floor_bytes"]:
        diagnostics.append("EVIDENCE_BUDGET_FREE_FLOOR_UNMET")
    value = report(
        schema_version=SCHEMA_VERSION, profile=profile_name, policy_sha256=policy_sha256,
        roots=roots, current=current, maximum=profile["maximum_bytes"],
        reserve=profile["reserve_bytes"], free_floor=profile["free_floor_bytes"],
        available=available, diagnostics=diagnostics,
        verdict="rejected" if diagnostics else "admitted",
    )
    return value, destination, profile, policy_sha256


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--policy", type=Path, default=Path("tools/evidence/retention-policy-v1.json"))
    parser.add_argument("--profile", default=PROFILE_ID)
    parser.add_argument("--destination", required=True)
    parser.add_argument("--create-destination", action="store_true")
    parser.add_argument("--postflight", action="store_true")
    arguments = parser.parse_args(argv)
    root = arguments.root.resolve()
    policy_path = arguments.policy if arguments.policy.is_absolute() else root / arguments.policy
    frozen = arguments.profile == PROFILE_ID
    try:
        policy_bytes, fallback_digest = load_policy_bytes(policy_path)
    except OSError:
        policy_bytes = None
        fallback_digest = None
    fallback = report(
        schema_version=POSTFLIGHT_SCHEMA_VERSION if arguments.postflight else SCHEMA_VERSION,
        profile=arguments.profile, policy_sha256=fallback_digest,
        roots=FROZEN_ROOTS if frozen else [], current=0,
        maximum=FROZEN_MAXIMUM if frozen else 0,
        reserve=FROZEN_RESERVE if frozen else 0,
        free_floor=FROZEN_FREE_FLOOR if frozen else 0,
        available=0, diagnostics=[], verdict="rejected",
    )
    try:
        if policy_bytes is None:
            raise BudgetError("EVIDENCE_BUDGET_INVALID_POLICY")
        if arguments.postflight:
            value, destination, profile, _digest = admission(
                root, arguments.profile, policy_bytes, fallback_digest, arguments.destination,
                destination_must_be_new=False, check_budget=False,
            )
            directory_fd = open_directory_under_root(root, destination, "EVIDENCE_BUDGET_UNSAFE_DESTINATION")
            try:
                diagnostics: list[str] = []
                try:
                    provenance = json.loads(read_regular_file(directory_fd, "preflight-report-v1.json", 65536))
                except (BudgetError, OSError, json.JSONDecodeError):
                    diagnostics.append("EVIDENCE_BUDGET_POLICY_PROVENANCE_MISMATCH")
                else:
                    if not isinstance(provenance, dict) or (
                        provenance.get("schema_version") != SCHEMA_VERSION
                        or provenance.get("verdict") != "admitted"
                        or provenance.get("profile") != arguments.profile
                        or provenance.get("policy_sha256") != fallback_digest
                    ):
                        diagnostics.append("EVIDENCE_BUDGET_POLICY_PROVENANCE_MISMATCH")
                root_device = os.fstat(directory_fd).st_dev
                if root_device != lstat_no_symlink(root, "EVIDENCE_BUDGET_UNSAFE_DESTINATION").st_dev:
                    raise BudgetError("EVIDENCE_BUDGET_MOUNT")
                total = int(value["current_bytes"])
                actual = scan_allocated_from_fd(root_device, directory_fd, profile)
                preliminary = report(
                    schema_version=POSTFLIGHT_SCHEMA_VERSION, profile=arguments.profile,
                    policy_sha256=value["policy_sha256"], roots=value["accounting_roots"], current=total,
                    maximum=profile["maximum_bytes"], reserve=profile["reserve_bytes"],
                    free_floor=profile["free_floor_bytes"], available=int(value["available_bytes"]),
                    diagnostics=diagnostics, verdict="rejected" if diagnostics else "within_reserve",
                )
                preliminary["actual_bundle_bytes"] = actual
                # Materialize the report before the authoritative scan so its
                # allocation is part of the retained bundle measurement.
                write_new_file(directory_fd, "postflight-report-v1.json", padded_report_payload(preliminary))
                os.fsync(directory_fd)
                actual = scan_allocated_from_fd(root_device, directory_fd, profile)
                total = sum(scan_allocated(root, safe_relative(root, item, "EVIDENCE_BUDGET_INVALID_POLICY"), profile)
                            for item in value["accounting_roots"])
                final_available = available_bytes_from_fd(directory_fd)
                if actual > profile["reserve_bytes"]:
                    diagnostics.append("EVIDENCE_BUDGET_ACTUAL_EXCEEDS_RESERVE")
                if total > profile["maximum_bytes"]:
                    diagnostics.append("EVIDENCE_BUDGET_POSTRUN_MAXIMUM_EXCEEDED")
                if final_available < profile["free_floor_bytes"]:
                    diagnostics.append("EVIDENCE_BUDGET_POSTRUN_FREE_FLOOR_UNMET")
                value = report(
                    schema_version=POSTFLIGHT_SCHEMA_VERSION, profile=arguments.profile,
                    policy_sha256=value["policy_sha256"], roots=value["accounting_roots"], current=total,
                    maximum=profile["maximum_bytes"], reserve=profile["reserve_bytes"],
                    free_floor=profile["free_floor_bytes"], available=final_available,
                    diagnostics=diagnostics, verdict="rejected" if diagnostics else "within_reserve",
                )
                value["actual_bundle_bytes"] = actual
                replace_file(directory_fd, "postflight-report-v1.json", padded_report_payload(value))
                # The replacement has the same padded size, but sample once
                # more from the anchored descriptor so the stored verdict is
                # never based on free space observed before its final report.
                observed_available = available_bytes_from_fd(directory_fd)
                if observed_available != final_available:
                    diagnostics = [code for code in diagnostics if code != "EVIDENCE_BUDGET_POSTRUN_FREE_FLOOR_UNMET"]
                    if observed_available < profile["free_floor_bytes"]:
                        diagnostics.append("EVIDENCE_BUDGET_POSTRUN_FREE_FLOOR_UNMET")
                    value = report(
                        schema_version=POSTFLIGHT_SCHEMA_VERSION, profile=arguments.profile,
                        policy_sha256=value["policy_sha256"], roots=value["accounting_roots"], current=total,
                        maximum=profile["maximum_bytes"], reserve=profile["reserve_bytes"],
                        free_floor=profile["free_floor_bytes"], available=observed_available,
                        diagnostics=diagnostics, verdict="rejected" if diagnostics else "within_reserve",
                    )
                    value["actual_bundle_bytes"] = actual
                    replace_file(directory_fd, "postflight-report-v1.json", padded_report_payload(value))
            finally:
                os.close(directory_fd)
            emit(value)
            return 2 if value["verdict"] == "rejected" else 0
        value, destination, _profile, digest = admission(
            root, arguments.profile, policy_bytes, fallback_digest, arguments.destination, destination_must_be_new=True,
        )
        if value["verdict"] != "admitted":
            emit(value)
            return 2
        if arguments.create_destination:
            directory_fd, parent_fd, temporary_name, destination_name, destination_device, destination_inode = secure_create_destination(root, destination)
            try:
                write_new_file(directory_fd, "preflight-report-v1.json", canonical_json(value).encode("utf-8"))
                write_new_file(directory_fd, "policy.sha256", f"{digest}  {policy_path.name}\n".encode("utf-8"))
                os.fsync(directory_fd)
                publish_destination(parent_fd, temporary_name, destination_name, destination_device, destination_inode)
            except BaseException:
                remove_created_destination(directory_fd, parent_fd, temporary_name, destination_device, destination_inode)
                os.close(directory_fd)
                directory_fd = None
                raise
            finally:
                if directory_fd is not None:
                    os.close(directory_fd)
                os.close(parent_fd)
        emit(value)
        return 0
    except BudgetError as error:
        fallback["diagnostics"] = [{"code": error.code}]
    except OSError:
        fallback["diagnostics"] = [{"code": "EVIDENCE_BUDGET_IO_ERROR"}]
    emit(fallback)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
