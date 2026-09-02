#!/usr/bin/env python3
"""Focused safety tests for tools/evidence/maintain_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import os
import sys
import tempfile
import time
import unittest
from contextlib import redirect_stderr, redirect_stdout
from io import StringIO
from pathlib import Path

MODULE = Path(__file__).parents[1] / "maintain_evidence.py"
SPEC = importlib.util.spec_from_file_location("maintain_evidence", MODULE)
assert SPEC and SPEC.loader
maintain = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = maintain
SPEC.loader.exec_module(maintain)


class EvidenceMaintenanceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.policy = self.root / "policy.json"
        self.write_policy()

    def tearDown(self) -> None:
        self.temp.cleanup()

    def write_policy(self, minimum_free_bytes: int = 1) -> None:
        self.policy.write_text(json.dumps({
            "schema_version": "hefaos.evidence-retention-policy.v1",
            "policy_id": "test",
            "scratch_relative_path": "target/evidence-maintenance",
            "minimum_free_bytes": minimum_free_bytes,
            "evidence_roots": ["evidence"],
            "cache_roots": ["target", "upstream-cache"],
            "state_file": ".hefaos-evidence-state.json",
            "active_marker": ".hefaos-evidence-active",
            "maximum_state_file_bytes": 65536,
            "archive_receipt_ledger": "evidence/archive-receipts-v1.json",
            "require_clean_tracked_archive_receipt_ledger": False,
            "minimum_recommendation_age_seconds": 0,
            "recommendable_states": ["failed", "partial"],
            "states": {name: name for name in ("accepted_external_verified", "review_pending", "failed", "partial")},
        }), encoding="utf-8")
        ledger = self.root / "evidence" / "archive-receipts-v1.json"
        ledger.parent.mkdir(parents=True, exist_ok=True)
        ledger.write_text(json.dumps({"schema_version": "hefaos.evidence-archive-receipts.v1", "receipts": []}), encoding="utf-8")

    def bind_receipt(self, candidate: Path) -> None:
        state_path = candidate / ".hefaos-evidence-state.json"
        state = json.loads(state_path.read_text(encoding="utf-8"))
        digest = maintain.canonical_tree_digest(candidate, json.loads(self.policy.read_text(encoding="utf-8")))
        state["evidence_tree_sha256"] = digest
        state_path.write_text(json.dumps(state), encoding="utf-8")
        self.write_receipt(str(state["archive_receipt_id"]), digest)

    def write_receipt(self, receipt_id: str, digest: str) -> None:
        ledger = self.root / "evidence" / "archive-receipts-v1.json"
        ledger.write_text(json.dumps({"schema_version": "hefaos.evidence-archive-receipts.v1", "receipts": [{
            "receipt_id": receipt_id, "evidence_tree_sha256": digest,
            "immutable_uri": "test://immutable-archive/fixture", "verified_at": "2026-09-02T00:00:00Z",
            "verification_manifest_sha256": "f" * 64,
        }]}), encoding="utf-8")

    def run_tool(self, *extra: str) -> tuple[int, dict[str, object], str]:
        out, err = StringIO(), StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            status = maintain.main(["--root", str(self.root), "--policy", str(self.policy), *extra])
        return status, json.loads(out.getvalue()) if out.getvalue() else {}, err.getvalue()

    def evidence_run(self, name: str, state: str | None, **fields: object) -> Path:
        path = self.root / "evidence" / "gate" / name
        path.mkdir(parents=True)
        if state is not None:
            record = {"schema_version": "hefaos.evidence-state.v1", "state": state, **fields}
            (path / ".hefaos-evidence-state.json").write_text(json.dumps(record), encoding="utf-8")
        return path

    def fixture_state(self) -> dict[str, object]:
        fixture = Path(__file__).parent / "fixtures" / "failed-externally-archived-eligible-v1.json"
        return json.loads(fixture.read_text(encoding="utf-8"))

    def test_default_dry_run_never_deletes_and_excludes_caches(self) -> None:
        state = self.fixture_state()
        candidate = self.evidence_run("failed", str(state.pop("state")), **state)
        self.bind_receipt(candidate)
        (self.root / "target" / "debug").mkdir(parents=True)
        status, report, _ = self.run_tool()
        self.assertEqual(status, 0)
        self.assertTrue(candidate.exists())
        self.assertEqual(report["mode"], "dry_run")
        self.assertEqual(report["counters"]["manual_recovery_candidates"], 1)
        self.assertEqual(report["counters"]["cache_roots_excluded"], 1)

    def test_unledgered_external_receipt_is_not_a_deletion_authorization(self) -> None:
        state = self.fixture_state()
        candidate = self.evidence_run("unledgered", str(state.pop("state")), **state)
        status, report, _ = self.run_tool()
        self.assertEqual(status, 0)
        self.assertTrue(candidate.exists())
        self.assertEqual(report["counters"]["manual_recovery_candidates"], 0)

    def test_recommendation_rejects_a_ledger_that_is_not_clean_and_tracked(self) -> None:
        state = self.fixture_state()
        candidate = self.evidence_run("untracked-ledger", str(state.pop("state")), **state)
        self.bind_receipt(candidate)
        policy = json.loads(self.policy.read_text(encoding="utf-8"))
        policy["require_clean_tracked_archive_receipt_ledger"] = True
        self.policy.write_text(json.dumps(policy), encoding="utf-8")
        status, _, error = self.run_tool("--recommend", "--confirm-recommendation", maintain.CONFIRMATION)
        self.assertEqual(status, 2)
        self.assertIn("EVIDENCE_RECEIPT_LEDGER_NOT_CLEAN_TRACKED", error)
        self.assertTrue(candidate.exists())

    def test_active_marker_mutation_after_eligibility_never_deletes(self) -> None:
        state = self.fixture_state()
        candidate = self.evidence_run("race", str(state.pop("state")), **state)
        self.bind_receipt(candidate)
        original_report_for = maintain.report_for

        def inject_active_after_scan(*args: object, **kwargs: object) -> dict[str, object]:
            report = original_report_for(*args, **kwargs)
            (candidate / ".hefaos-evidence-active").touch()
            return report

        maintain.report_for = inject_active_after_scan
        try:
            status, report, _ = self.run_tool("--recommend", "--confirm-recommendation", maintain.CONFIRMATION)
        finally:
            maintain.report_for = original_report_for
        self.assertEqual(status, 0)
        self.assertTrue(candidate.exists())
        self.assertNotIn("EVIDENCE_MANUAL_RECOVERY_RECOMMENDED", [item["code"] for item in report["diagnostics"]])
        self.assertIn("EVIDENCE_FULL_REVIEW_REQUIRED", [item["code"] for item in report["diagnostics"]])

    def test_confirmed_recommendation_never_removes_an_eligible_evidence_path(self) -> None:
        state = self.fixture_state()
        candidate = self.evidence_run("rmtree-failure", str(state.pop("state")), **state)
        self.bind_receipt(candidate)
        status, report, _ = self.run_tool("--recommend", "--confirm-recommendation", maintain.CONFIRMATION)
        self.assertEqual(status, 0)
        self.assertIn("EVIDENCE_MANUAL_RECOVERY_RECOMMENDED", [item["code"] for item in report["diagnostics"]])
        self.assertTrue(candidate.exists())

    def test_recommendation_requires_explicit_confirmation_then_retains_only_eligible(self) -> None:
        candidate = self.evidence_run("partial", "partial", archive_scope="external_verified", manual_recovery_eligible=True,
                                      archive_receipt_id="partial-receipt", evidence_tree_sha256="")
        self.bind_receipt(candidate)
        status, _, error = self.run_tool("--recommend")
        self.assertEqual(status, 2)
        self.assertIn("EVIDENCE_CONFIRMATION_REQUIRED", error)
        self.assertTrue(candidate.exists())
        status, report, _ = self.run_tool("--recommend", "--confirm-recommendation", maintain.CONFIRMATION)
        self.assertEqual(status, 0)
        self.assertTrue(candidate.exists())
        self.assertIn("EVIDENCE_MANUAL_RECOVERY_RECOMMENDED", [item["code"] for item in report["diagnostics"]])

    def test_accepted_review_unknown_active_and_symlink_entries_are_protected(self) -> None:
        accepted = self.evidence_run("accepted", "accepted_external_verified", archive_scope="external_verified")
        review = self.evidence_run("review", "review_pending", archive_scope="local_only")
        unknown = self.evidence_run("unknown", None)
        active = self.evidence_run("active", "failed", archive_scope="local_only", manual_recovery_eligible=True)
        (active / ".hefaos-evidence-active").touch()
        link = self.root / "evidence" / "gate" / "linked"
        try:
            link.symlink_to(self.root / "outside")
        except OSError as error:
            self.skipTest(f"symlinks unavailable: {error}")
        status, report, _ = self.run_tool()
        self.assertEqual(status, 0)
        self.assertTrue(all(path.exists() for path in (accepted, review, unknown, active)))
        counters = report["counters"]
        self.assertEqual(counters["accepted_protected"], 1)
        self.assertEqual(counters["review_pending_protected"], 1)
        self.assertEqual(counters["unknown_protected"], 1)
        self.assertEqual(counters["active_protected"], 1)
        self.assertEqual(counters["symlink_or_out_of_root_protected"], 1)

    def test_dangling_active_marker_and_nonregular_or_malformed_state_are_protected(self) -> None:
        active = self.evidence_run("dangling-active", "failed", archive_scope="external_verified", manual_recovery_eligible=True)
        (active / ".hefaos-evidence-active").symlink_to("missing-owner")
        fifo = self.evidence_run("fifo-state", None)
        os.mkfifo(fifo / ".hefaos-evidence-state.json")
        list_state = self.evidence_run("list-state", None)
        (list_state / ".hefaos-evidence-state.json").write_text("[]", encoding="utf-8")
        status, report, _ = self.run_tool()
        self.assertEqual(status, 0)
        self.assertTrue(active.exists())
        self.assertTrue(fifo.exists())
        self.assertTrue(list_state.exists())
        self.assertEqual(report["counters"]["active_protected"], 1)
        self.assertEqual(report["counters"]["unknown_protected"], 2)

    def test_unwritable_scratch_preflight_prevents_deletion(self) -> None:
        state = self.fixture_state()
        candidate = self.evidence_run("failed", str(state.pop("state")), **state)
        self.bind_receipt(candidate)
        self.write_policy()
        (self.root / "target").write_text("not a directory", encoding="utf-8")
        status, _, _ = self.run_tool("--recommend", "--confirm-recommendation", maintain.CONFIRMATION)
        self.assertEqual(status, 2)
        self.assertTrue(candidate.exists())

    def test_state_filename_traversal_is_rejected_before_discovery(self) -> None:
        state = self.fixture_state()
        candidate = self.evidence_run("failed", str(state.pop("state")), **state)
        self.bind_receipt(candidate)
        policy = json.loads(self.policy.read_text(encoding="utf-8"))
        policy["state_file"] = "../shared-state.json"
        self.policy.write_text(json.dumps(policy), encoding="utf-8")
        status, _, error = self.run_tool("--recommend", "--confirm-recommendation", maintain.CONFIRMATION)
        self.assertEqual(status, 2)
        self.assertIn("state_file must be a single filename", error)
        self.assertTrue(candidate.exists())

    def test_insufficient_space_fails_closed_before_scratch_or_deletion(self) -> None:
        candidate = self.evidence_run("failed", "failed", archive_scope="external_verified", manual_recovery_eligible=True,
                                      archive_receipt_id="space-receipt", evidence_tree_sha256="")
        self.bind_receipt(candidate)
        self.write_policy(1 << 62)
        status, _, error = self.run_tool("--recommend", "--confirm-recommendation", maintain.CONFIRMATION)
        self.assertEqual(status, 2)
        self.assertIn("EVIDENCE_INSUFFICIENT_SPACE", error)
        self.assertTrue(candidate.exists())
        self.assertFalse((self.root / "target" / "evidence-maintenance").exists())

    def test_manifest_framing_distinguishes_path_and_content_boundaries(self) -> None:
        first = self.evidence_run("first", None)
        second = self.evidence_run("second", None)
        (first / "a").write_bytes(b"bc")
        (second / "ab").write_bytes(b"c")
        policy = json.loads(self.policy.read_text(encoding="utf-8"))
        self.assertNotEqual(maintain.canonical_tree_digest(first, policy), maintain.canonical_tree_digest(second, policy))

    def test_nested_metadata_named_file_is_included_in_manifest(self) -> None:
        candidate = self.evidence_run("nested", None)
        nested = candidate / "nested"
        nested.mkdir()
        (nested / ".hefaos-evidence-state.json").write_bytes(b"one")
        policy = json.loads(self.policy.read_text(encoding="utf-8"))
        before = maintain.canonical_tree_digest(candidate, policy)
        (nested / ".hefaos-evidence-state.json").write_bytes(b"two")
        self.assertNotEqual(before, maintain.canonical_tree_digest(candidate, policy))

    def test_scratch_inside_evidence_is_rejected_before_report_or_recommendation(self) -> None:
        state = self.fixture_state()
        candidate = self.evidence_run("candidate", str(state.pop("state")), **state)
        self.bind_receipt(candidate)
        policy = json.loads(self.policy.read_text(encoding="utf-8"))
        policy["scratch_relative_path"] = "evidence/gate/candidate"
        self.policy.write_text(json.dumps(policy), encoding="utf-8")
        status, _, error = self.run_tool("--recommend", "--confirm-recommendation", maintain.CONFIRMATION)
        self.assertEqual(status, 2)
        self.assertIn("scratch_relative_path must", error)
        self.assertTrue(candidate.exists())

    def test_scratch_symlink_alias_is_rejected_before_report_write(self) -> None:
        source = self.root / "evidence" / "report-source"
        source.mkdir(parents=True)
        (source / "last-maintenance-report-v1.json").write_text("preserve", encoding="utf-8")
        (self.root / "target").symlink_to(source, target_is_directory=True)
        status, _, error = self.run_tool()
        self.assertEqual(status, 2)
        self.assertIn("must not traverse a symlink", error)
        self.assertEqual((source / "last-maintenance-report-v1.json").read_text(encoding="utf-8"), "preserve")

    def test_scratch_swap_after_validation_cannot_redirect_report_write(self) -> None:
        source = self.root / "evidence" / "swap-source"
        source.mkdir(parents=True)
        report = source / "last-maintenance-report-v1.json"
        report.write_text("preserve", encoding="utf-8")
        original_ensure = maintain.ensure_space
        calls = 0

        def swap_after_safe_open(descriptor: int, minimum: int) -> None:
            nonlocal calls
            calls += 1
            if calls == 2:
                os.rename(self.root / "target", self.root / "target-real")
                (self.root / "target").symlink_to(source, target_is_directory=True)
            original_ensure(descriptor, minimum)

        maintain.ensure_space = swap_after_safe_open
        try:
            status, _, _ = self.run_tool()
        finally:
            maintain.ensure_space = original_ensure
        self.assertEqual(status, 0)
        self.assertEqual(report.read_text(encoding="utf-8"), "preserve")

    def test_malformed_state_and_truthy_eligibility_are_protected(self) -> None:
        malformed = self.evidence_run("malformed", None)
        (malformed / ".hefaos-evidence-state.json").write_text(json.dumps({
            "schema_version": "hefaos.evidence-state.v1", "state": []
        }), encoding="utf-8")
        state = self.fixture_state()
        state["manual_recovery_eligible"] = "true"
        truthy = self.evidence_run("truthy", str(state.pop("state")), **state)
        self.bind_receipt(truthy)
        status, report, _ = self.run_tool()
        self.assertEqual(status, 0)
        self.assertTrue(malformed.exists() and truthy.exists())
        self.assertEqual(report["counters"]["manual_recovery_candidates"], 0)

    def test_nonobject_and_invalid_policy_fields_fail_with_status_two(self) -> None:
        self.policy.write_text("[]", encoding="utf-8")
        status, _, error = self.run_tool()
        self.assertEqual(status, 2)
        self.assertIn("policy must be a JSON object", error)
        self.write_policy()
        policy = json.loads(self.policy.read_text(encoding="utf-8"))
        policy["minimum_free_bytes"] = "64"
        self.policy.write_text(json.dumps(policy), encoding="utf-8")
        status, _, error = self.run_tool()
        self.assertEqual(status, 2)
        self.assertIn("minimum_free_bytes must be a nonnegative integer", error)
        self.write_policy()
        policy = json.loads(self.policy.read_text(encoding="utf-8"))
        policy["state_file"] = []
        self.policy.write_text(json.dumps(policy), encoding="utf-8")
        status, _, error = self.run_tool()
        self.assertEqual(status, 2)
        self.assertIn("state_file must be a nonempty string", error)

    def test_report_writer_handles_short_writes(self) -> None:
        scratch = self.root / "target" / "evidence-maintenance"
        descriptor = maintain.open_scratch_dir(self.root, scratch)
        original_write = maintain.os.write

        def short_write(fd: int, payload: bytes | memoryview) -> int:
            return original_write(fd, payload[:1])

        maintain.os.write = short_write
        try:
            maintain.write_report(descriptor, {"schema_version": "test"})
        finally:
            maintain.os.write = original_write
            os.close(descriptor)
        report = scratch / "last-maintenance-report-v1.json"
        self.assertEqual(json.loads(report.read_text(encoding="utf-8"))["schema_version"], "test")

    def test_report_writer_nonprogress_cleans_anchored_temp_and_preserves_final(self) -> None:
        scratch = self.root / "target" / "evidence-maintenance"
        descriptor = maintain.open_scratch_dir(self.root, scratch)
        final = scratch / "last-maintenance-report-v1.json"
        final.write_text("prior", encoding="utf-8")
        original_write = maintain.os.write
        maintain.os.write = lambda _fd, _payload: 0
        try:
            with self.assertRaisesRegex(OSError, "EVIDENCE_REPORT_SHORT_WRITE"):
                maintain.write_report(descriptor, {"schema_version": "test"})
        finally:
            maintain.os.write = original_write
            os.close(descriptor)
        self.assertEqual(final.read_text(encoding="utf-8"), "prior")
        self.assertEqual([path.name for path in scratch.iterdir()], [final.name])

    def test_policy_rejects_mystery_or_reordered_v1_states(self) -> None:
        policy = json.loads(self.policy.read_text(encoding="utf-8"))
        policy["states"]["mystery"] = "mystery"
        self.policy.write_text(json.dumps(policy), encoding="utf-8")
        status, _, error = self.run_tool()
        self.assertEqual(status, 2)
        self.assertIn("states must be an object", error)
        self.write_policy()
        policy = json.loads(self.policy.read_text(encoding="utf-8"))
        policy["recommendable_states"] = ["partial", "failed"]
        self.policy.write_text(json.dumps(policy), encoding="utf-8")
        status, _, error = self.run_tool()
        self.assertEqual(status, 2)
        self.assertIn("recommendable_states must be exactly", error)


if __name__ == "__main__":
    unittest.main()
