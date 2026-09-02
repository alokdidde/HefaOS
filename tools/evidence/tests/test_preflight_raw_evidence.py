"""Focused adversarial checks for the frozen Gate 0 raw-evidence budget."""

from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock
from pathlib import Path
from contextlib import redirect_stdout
from types import SimpleNamespace


TOOL = Path(__file__).parents[1] / "preflight_raw_evidence.py"
REPOSITORY_ROOT = TOOL.parents[2]
RUNNER = REPOSITORY_ROOT / "testbench/tools/run-gate-0-copper-evidence.sh"
SPEC = importlib.util.spec_from_file_location("preflight_raw_evidence", TOOL)
assert SPEC is not None and SPEC.loader is not None
BUDGET = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUDGET)


class RawEvidenceBudgetTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        (self.root / "evidence/gate-0-copper").mkdir(parents=True)
        (self.root / "target").mkdir()
        self.policy_path = self.root / "policy.json"
        self.write_policy()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write_policy(self, *, frozen: bool = True, reserve: int = 65536, maximum: int = 131072) -> None:
        profile = {
            "accounting_roots": ["evidence/gate-0-copper", "target/gate-0-copper-evidence"],
            "maximum_bytes": 4294967296 if frozen else maximum,
            "reserve_bytes": 3221225472 if frozen else reserve,
            "free_floor_bytes": 1073741824 if frozen else 1,
            "maximum_scan_entries": 100,
            "maximum_scan_depth": 8,
            "maximum_path_bytes": 256,
        }
        policy = {"schema_version": "hefaos.evidence-retention-policy.v1", "raw_evidence_budget_profiles": {
            "gate-0-copper-raw-v1": profile if frozen else {
                **profile, "maximum_bytes": 4294967296, "reserve_bytes": 3221225472, "free_floor_bytes": 1073741824,
            },
            "test": profile,
        }}
        self.policy_path.write_text(json.dumps(policy), encoding="utf-8")

    def invoke(self, *arguments: str) -> tuple[int, dict[str, object]]:
        command = [sys.executable, str(TOOL), "--root", str(self.root), "--policy", str(self.policy_path), *arguments]
        result = subprocess.run(command, check=False, capture_output=True, text=True)
        return result.returncode, json.loads(result.stdout)

    def test_frozen_profile_rejects_changed_constants(self) -> None:
        policy = json.loads(self.policy_path.read_text(encoding="utf-8"))
        policy["raw_evidence_budget_profiles"]["gate-0-copper-raw-v1"]["reserve_bytes"] = 1
        self.policy_path.write_text(json.dumps(policy), encoding="utf-8")
        status, report = self.invoke("--destination", "target/gate-0-copper-evidence/new")
        self.assertEqual(status, 2)
        self.assertEqual(report["diagnostics"], [{"code": "EVIDENCE_BUDGET_FROZEN_PROFILE_MISMATCH"}])

    def test_threshold_rejection_does_not_create_destination_or_touch_sentinel(self) -> None:
        self.write_policy(frozen=False, reserve=4096, maximum=4096)
        (self.root / "evidence/gate-0-copper/raw.bin").write_bytes(b"x" * 4096)
        sentinel = self.root / "sentinel"
        sentinel.write_text("unchanged", encoding="utf-8")
        destination = "target/gate-0-copper-evidence/rejected"
        status, report = self.invoke("--profile", "test", "--destination", destination, "--create-destination")
        self.assertEqual(status, 2)
        self.assertEqual(report["verdict"], "rejected")
        self.assertIn({"code": "EVIDENCE_BUDGET_MAXIMUM_EXCEEDED"}, report["diagnostics"])
        self.assertFalse((self.root / destination).exists())
        self.assertEqual(sentinel.read_text(encoding="utf-8"), "unchanged")

    def test_symlinked_accounting_ancestor_is_rejected(self) -> None:
        outside = self.root / "outside/gate-0-copper"
        outside.mkdir(parents=True)
        (outside / "raw.bin").write_bytes(b"evidence")
        (self.root / "evidence/gate-0-copper").rmdir()
        (self.root / "evidence").rmdir()
        (self.root / "evidence").symlink_to(self.root / "outside")
        status, report = self.invoke("--destination", "target/gate-0-copper-evidence/new")
        self.assertEqual(status, 2)
        self.assertEqual(report["diagnostics"], [{"code": "EVIDENCE_BUDGET_UNSAFE_ROOT"}])

    def test_scanner_does_not_follow_a_directory_swapped_to_symlink(self) -> None:
        account = self.root / "evidence/gate-0-copper"
        nested = account / "sub"
        nested.mkdir()
        (nested / "local.bin").write_bytes(b"local")
        outside = self.root / "outside"
        outside.mkdir()
        (outside / "escaped.bin").write_bytes(b"outside")
        original_open = BUDGET.os.open
        swapped = False

        def swap_before_open(name: object, flags: int, *args: object, **kwargs: object) -> int:
            nonlocal swapped
            if name == "sub" and kwargs.get("dir_fd") is not None and not swapped:
                swapped = True
                nested.rename(account / "sub-old")
                nested.symlink_to(outside, target_is_directory=True)
            return original_open(name, flags, *args, **kwargs)

        profile = {"maximum_scan_entries": 100, "maximum_scan_depth": 8, "maximum_path_bytes": 256}
        with mock.patch.object(BUDGET.os, "open", side_effect=swap_before_open):
            with self.assertRaises(BUDGET.BudgetError) as error:
                BUDGET.scan_allocated(self.root, account, profile)
        self.assertIn(error.exception.code, {"EVIDENCE_BUDGET_SCAN_RACE", "EVIDENCE_BUDGET_SYMLINK"})

    def test_existing_destination_is_never_overwritten(self) -> None:
        destination = self.root / "target/gate-0-copper-evidence/existing"
        destination.mkdir(parents=True)
        (destination / "keep").write_text("keep", encoding="utf-8")
        status, report = self.invoke("--destination", str(destination), "--create-destination")
        self.assertEqual(status, 2)
        self.assertEqual(report["diagnostics"], [{"code": "EVIDENCE_BUDGET_DESTINATION_EXISTS"}])
        self.assertEqual((destination / "keep").read_text(encoding="utf-8"), "keep")

    def test_provenance_write_failure_removes_only_the_new_bundle(self) -> None:
        self.write_policy(frozen=False)
        destination = self.root / "target/gate-0-copper-evidence/provenance-failure"
        with mock.patch.object(BUDGET, "write_new_file", side_effect=OSError("injected")):
            with redirect_stdout(io.StringIO()):
                status = BUDGET.main([
                    "--root", str(self.root), "--policy", str(self.policy_path), "--profile", "test",
                    "--destination", str(destination), "--create-destination",
                ])
        self.assertEqual(status, 2)
        self.assertFalse(destination.exists())

    def test_provenance_cleanup_does_not_touch_a_renamed_destination_replacement(self) -> None:
        self.write_policy(frozen=False)
        destination = self.root / "target/gate-0-copper-evidence/cleanup-swap"
        original_write = BUDGET.write_new_file

        def swap_then_fail(directory_fd: int, name: str, payload: bytes) -> None:
            if name == "preflight-report-v1.json":
                original_write(directory_fd, name, payload)
                return
            destination.mkdir()
            (destination / "replacement-sentinel").write_text("keep", encoding="utf-8")
            raise OSError("injected")

        with mock.patch.object(BUDGET, "write_new_file", side_effect=swap_then_fail):
            with redirect_stdout(io.StringIO()):
                status = BUDGET.main([
                    "--root", str(self.root), "--policy", str(self.policy_path), "--profile", "test",
                    "--destination", str(destination), "--create-destination",
                ])
        self.assertEqual(status, 2)
        self.assertEqual((destination / "replacement-sentinel").read_text(encoding="utf-8"), "keep")
        staging = list(destination.parent.glob(".gate-0-copper-admission-*.tmp"))
        self.assertEqual(len(staging), 1)
        self.assertTrue((staging[0] / "preflight-report-v1.json").exists())

    def test_exclusive_destination_allows_repeated_ancestor_basename(self) -> None:
        self.write_policy(frozen=False)
        parent = self.root / "target/gate-0-copper-evidence/repeated"
        parent.mkdir(parents=True)
        destination = parent / "repeated"
        status, report = self.invoke("--profile", "test", "--destination", str(destination), "--create-destination")
        self.assertEqual(status, 0)
        self.assertEqual(report["verdict"], "admitted")
        self.assertTrue((destination / "preflight-report-v1.json").exists())

    def test_staging_swap_before_open_cannot_redirect_provenance_to_final(self) -> None:
        self.write_policy(frozen=False)
        managed_root = self.root / "target/gate-0-copper-evidence"
        destination = managed_root / "swap-before-open"
        outside = self.root / "outside"
        outside.mkdir()
        original_open = BUDGET.os.open
        swapped = False

        def swap_before_open(name: object, flags: int, *args: object, **kwargs: object) -> int:
            nonlocal swapped
            if isinstance(name, str) and name.startswith(".gate-0-copper-admission-") and not swapped:
                swapped = True
                staging = managed_root / name
                staging.rename(managed_root / "staging-moved")
                staging.symlink_to(outside, target_is_directory=True)
            return original_open(name, flags, *args, **kwargs)

        with mock.patch.object(BUDGET.os, "open", side_effect=swap_before_open):
            with redirect_stdout(io.StringIO()):
                status = BUDGET.main([
                    "--root", str(self.root), "--policy", str(self.policy_path), "--profile", "test",
                    "--destination", str(destination), "--create-destination",
                ])
        self.assertEqual(status, 2)
        self.assertFalse(destination.exists())
        self.assertFalse((outside / "preflight-report-v1.json").exists())

    def test_publication_rejects_regular_directory_staging_swap(self) -> None:
        parent = self.root / "target/publication"
        parent.mkdir()
        staging = parent / "staging"
        final = "final"
        staging.mkdir()
        original = staging.stat()
        staging.rename(parent / "staging-original")
        staging.mkdir()
        parent_fd = os.open(parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            with self.assertRaises(BUDGET.BudgetError) as error:
                BUDGET.publish_destination(parent_fd, "staging", final, original.st_dev, original.st_ino)
        finally:
            os.close(parent_fd)
        self.assertEqual(error.exception.code, "EVIDENCE_BUDGET_PUBLICATION_IDENTITY_MISMATCH")
        self.assertFalse((parent / final).exists())
        self.assertTrue((parent / "staging-original").is_dir())

    def test_publication_rejects_symlink_staging_swap(self) -> None:
        parent = self.root / "target/publication"
        parent.mkdir()
        staging = parent / "staging"
        final = "final"
        staging.mkdir()
        original = staging.stat()
        staging.rename(parent / "staging-original")
        staging.symlink_to(self.root / "outside", target_is_directory=True)
        parent_fd = os.open(parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            with self.assertRaises(BUDGET.BudgetError) as error:
                BUDGET.publish_destination(parent_fd, "staging", final, original.st_dev, original.st_ino)
        finally:
            os.close(parent_fd)
        self.assertEqual(error.exception.code, "EVIDENCE_BUDGET_PUBLICATION_IDENTITY_MISMATCH")
        self.assertFalse((parent / final).exists())
        self.assertTrue((parent / "staging-original").is_dir())

    def test_publication_rejects_final_name_substitution_after_rename(self) -> None:
        parent = self.root / "target/publication"
        parent.mkdir()
        staging = parent / "staging"
        final = "final"
        outside = self.root / "outside"
        outside.mkdir()
        staging.mkdir()
        original = staging.stat()
        parent_fd = os.open(parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        original_stat = BUDGET.os.stat
        swapped = False

        def substitute_before_final_stat(name: object, *args: object, **kwargs: object) -> os.stat_result:
            nonlocal swapped
            if name == final and kwargs.get("dir_fd") == parent_fd and not swapped:
                swapped = True
                published = parent / final
                published.rename(parent / "published-original")
                published.symlink_to(outside, target_is_directory=True)
            return original_stat(name, *args, **kwargs)

        try:
            with mock.patch.object(BUDGET.os, "stat", side_effect=substitute_before_final_stat):
                with self.assertRaises(BUDGET.BudgetError) as error:
                    BUDGET.publish_destination(parent_fd, "staging", final, original.st_dev, original.st_ino)
        finally:
            os.close(parent_fd)
        self.assertEqual(error.exception.code, "EVIDENCE_BUDGET_PUBLICATION_IDENTITY_MISMATCH")
        self.assertTrue((parent / "published-original").is_dir())
        self.assertTrue((parent / final).is_symlink())
        self.assertFalse((outside / "preflight-report-v1.json").exists())

    def test_admitted_report_and_digest_are_retained_and_postflight_counts_report(self) -> None:
        self.write_policy(frozen=False, reserve=32768, maximum=65536)
        destination = self.root / "target/gate-0-copper-evidence/new"
        status, report = self.invoke("--profile", "test", "--destination", str(destination), "--create-destination")
        self.assertEqual(status, 0)
        self.assertEqual(report["verdict"], "admitted")
        stored = json.loads((destination / "preflight-report-v1.json").read_text(encoding="utf-8"))
        self.assertEqual(stored, report)
        digest = hashlib.sha256(self.policy_path.read_bytes()).hexdigest()
        self.assertEqual((destination / "policy.sha256").read_text(encoding="utf-8"), f"{digest}  policy.json\n")
        # This allocation plus the padded postflight report exceeds the 32 KiB
        # reservation; the failure must retain the evidence and its report.
        (destination / "raw.bin").write_bytes(b"x" * 32768)
        status, postflight = self.invoke("--profile", "test", "--destination", str(destination), "--postflight")
        self.assertEqual(status, 2)
        self.assertEqual(postflight["verdict"], "rejected")
        self.assertIn({"code": "EVIDENCE_BUDGET_ACTUAL_EXCEEDS_RESERVE"}, postflight["diagnostics"])
        self.assertTrue((destination / "postflight-report-v1.json").exists())
        self.assertGreater(int(postflight["actual_bundle_bytes"]), 32768)

    def test_postflight_rejects_policy_changed_after_admission(self) -> None:
        self.write_policy(frozen=False)
        destination = self.root / "target/gate-0-copper-evidence/policy-changed"
        status, _report = self.invoke("--profile", "test", "--destination", str(destination), "--create-destination")
        self.assertEqual(status, 0)
        policy = json.loads(self.policy_path.read_text(encoding="utf-8"))
        policy["raw_evidence_budget_profiles"]["test"]["maximum_bytes"] += 4096
        self.policy_path.write_text(json.dumps(policy), encoding="utf-8")
        status, report = self.invoke("--profile", "test", "--destination", str(destination), "--postflight")
        self.assertEqual(status, 2)
        self.assertIn({"code": "EVIDENCE_BUDGET_POLICY_PROVENANCE_MISMATCH"}, report["diagnostics"])
        self.assertTrue((destination / "postflight-report-v1.json").exists())

    def test_postflight_resamples_final_free_space_and_rejects_floor_crossing(self) -> None:
        self.write_policy(frozen=False)
        destination = self.root / "target/gate-0-copper-evidence/free-floor"
        status, _report = self.invoke("--profile", "test", "--destination", str(destination), "--create-destination")
        self.assertEqual(status, 0)
        output = io.StringIO()
        with mock.patch.object(BUDGET.os, "fstatvfs", return_value=SimpleNamespace(f_bavail=0, f_frsize=1)):
            with redirect_stdout(output):
                status = BUDGET.main([
                    "--root", str(self.root), "--policy", str(self.policy_path), "--profile", "test",
                    "--destination", str(destination), "--postflight",
                ])
        report = json.loads(output.getvalue())
        self.assertEqual(status, 2)
        self.assertEqual(report["available_bytes"], 0)
        self.assertIn({"code": "EVIDENCE_BUDGET_POSTRUN_FREE_FLOOR_UNMET"}, report["diagnostics"])

    def test_postflight_anchored_fd_cannot_be_redirected_by_ancestor_swap(self) -> None:
        self.write_policy(frozen=False)
        destination = self.root / "target/gate-0-copper-evidence/anchored"
        status, _report = self.invoke("--profile", "test", "--destination", str(destination), "--create-destination")
        self.assertEqual(status, 0)
        managed_root = self.root / "target/gate-0-copper-evidence"
        moved_root = self.root / "target/gate-0-copper-evidence-moved"
        outside = self.root / "outside"
        outside.mkdir()
        original_write = BUDGET.write_new_file
        swapped = False

        def swap_before_write(directory_fd: int, name: str, payload: bytes) -> None:
            nonlocal swapped
            if name == "postflight-report-v1.json" and not swapped:
                swapped = True
                managed_root.rename(moved_root)
                managed_root.symlink_to(outside, target_is_directory=True)
            original_write(directory_fd, name, payload)

        with mock.patch.object(BUDGET, "write_new_file", side_effect=swap_before_write):
            with redirect_stdout(io.StringIO()):
                status = BUDGET.main([
                    "--root", str(self.root), "--policy", str(self.policy_path), "--profile", "test",
                    "--destination", str(destination), "--postflight",
                ])
        self.assertEqual(status, 2)
        self.assertTrue((moved_root / "anchored/postflight-report-v1.json").exists())
        self.assertFalse((outside / "anchored/postflight-report-v1.json").exists())

    def test_runner_lock_rejects_before_preflight_or_workload(self) -> None:
        """The full runner uses a nonmutating, stable repository lock anchor."""
        destination = REPOSITORY_ROOT / "target/gate-0-copper-evidence/lock-test-never-created"
        self.assertFalse(destination.exists())
        holder = subprocess.Popen(
            ["bash", "-c", "exec 9<\"$1\"; flock -n 9; sleep 2", "--", str(REPOSITORY_ROOT)],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        try:
            time.sleep(0.1)
            result = subprocess.run(
                ["bash", str(RUNNER)], check=False, capture_output=True, text=True,
                env={**__import__("os").environ, "HEFAOS_GATE0_EVIDENCE_DIR": str(destination)}, timeout=1,
            )
        finally:
            holder.terminate()
            holder.wait(timeout=2)
        self.assertEqual(result.returncode, 2)
        self.assertIn("EVIDENCE_BUDGET_PRODUCER_LOCKED", result.stderr)
        self.assertFalse(destination.exists())

    def test_repository_lock_survives_atomic_policy_replacement(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            runner = root / "testbench/tools/run-gate-0-copper-evidence.sh"
            policy = root / "tools/evidence/retention-policy-v1.json"
            tool = root / "tools/evidence/preflight_raw_evidence.py"
            runner.parent.mkdir(parents=True)
            policy.parent.mkdir(parents=True)
            shutil.copy2(RUNNER, runner)
            shutil.copy2(REPOSITORY_ROOT / "tools/evidence/retention-policy-v1.json", policy)
            shutil.copy2(TOOL, tool)
            replacement = policy.with_name("replacement.json")
            replacement.write_bytes(policy.read_bytes())
            holder = subprocess.Popen(
                ["bash", "-c", "exec 9<\"$1\"; flock -n 9; sleep 2", "--", str(root)],
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            )
            try:
                time.sleep(0.1)
                os.replace(replacement, policy)
                destination = root / "target/gate-0-copper-evidence/never-created"
                result = subprocess.run(
                    ["bash", str(runner)], check=False, capture_output=True, text=True,
                    env={**os.environ, "HEFAOS_GATE0_EVIDENCE_DIR": str(destination)}, timeout=1,
                )
            finally:
                holder.terminate()
                holder.wait(timeout=2)
            self.assertEqual(result.returncode, 2)
            self.assertIn("EVIDENCE_BUDGET_PRODUCER_LOCKED", result.stderr)
            self.assertFalse(destination.exists())


if __name__ == "__main__":
    unittest.main()
