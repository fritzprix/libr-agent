#!/usr/bin/env python3
"""Tests for latest-run-per-check selection.

Run: python3 .agents/skills/loop-shared/scripts/test_ci_checks.py
"""
from __future__ import annotations

import unittest
from pathlib import Path
import sys

SCRIPTS = Path(__file__).resolve().parent
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from ci_checks import check_state, check_run_ts, effective_checks  # noqa: E402


def run(name, conclusion="", status="", started=None, created=None, completed=None, workflow=None, context=None):
    item = {}
    if name is not None:
        item["name"] = name
    if context is not None:
        item["context"] = context
    if conclusion:
        item["conclusion"] = conclusion
    if status:
        item["status"] = status
    if started is not None:
        item["startedAt"] = started
    if created is not None:
        item["createdAt"] = created
    if completed is not None:
        item["completedAt"] = completed
    if workflow is not None:
        item["workflowName"] = workflow
    return item


class CheckRunTsTests(unittest.TestCase):
    def test_prefers_started_then_created_then_nonzero_completed(self) -> None:
        self.assertEqual(check_run_ts({"startedAt": "2026-11-27T01:00:00Z"}), "2026-11-27T01:00:00Z")
        self.assertEqual(
            check_run_ts({"createdAt": "2026-11-27T01:00:00Z", "completedAt": "2026-11-27T01:02:00Z"}),
            "2026-11-27T01:00:00Z",
        )
        self.assertEqual(check_run_ts({"completedAt": "2026-11-27T01:02:00Z"}), "2026-11-27T01:02:00Z")

    def test_zero_completed_at_is_missing(self) -> None:
        self.assertEqual(
            check_run_ts({"status": "IN_PROGRESS", "completedAt": "0001-01-01T00:00:00Z"}),
            "",
        )
        self.assertEqual(
            check_run_ts({
                "startedAt": "2026-11-27T01:05:00Z",
                "completedAt": "0001-01-01T00:00:00Z",
            }),
            "2026-11-27T01:05:00Z",
        )


class EffectiveChecksTests(unittest.TestCase):
    def test_older_cancelled_or_failure_then_newer_success_keeps_success(self) -> None:
        checks = [
            run("Test", "SUCCESS", started="2026-11-27T01:00:00Z", completed="2026-11-27T01:02:00Z"),
            run("Build", "CANCELLED", started="2026-11-27T01:00:00Z", completed="2026-11-27T01:01:00Z"),
            run("Build", "FAILURE", started="2026-11-27T01:02:00Z", completed="2026-11-27T01:03:00Z"),
            run("Build", "SUCCESS", started="2026-11-27T01:05:00Z", completed="2026-11-27T01:07:00Z"),
        ]
        eff = effective_checks(checks)
        self.assertEqual(
            [(c["name"], check_state(c)) for c in eff],
            [("Test", "SUCCESS"), ("Build", "SUCCESS")],
        )

    def test_older_success_then_newer_in_progress_is_pending(self) -> None:
        checks = [
            run("Test", "SUCCESS", started="2026-11-27T01:00:00Z", completed="2026-11-27T01:02:00Z"),
            run(
                "Test",
                status="IN_PROGRESS",
                started="2026-11-27T01:05:00Z",
                completed="0001-01-01T00:00:00Z",
            ),
        ]
        eff = effective_checks(checks)
        self.assertEqual(len(eff), 1)
        self.assertEqual(check_state(eff[0]), "IN_PROGRESS")

    def test_same_name_in_two_workflows_is_independent(self) -> None:
        checks = [
            run("test", "FAILURE", started="2026-11-27T01:00:00Z", workflow="Loop skill tests"),
            run("test", "SUCCESS", started="2026-11-27T01:00:00Z", workflow="LibrAgent CI/CD"),
            run("test", "CANCELLED", started="2026-11-27T00:50:00Z", workflow="LibrAgent CI/CD"),
        ]
        eff = effective_checks(checks)
        by_workflow = {c["workflowName"]: check_state(c) for c in eff}
        self.assertEqual(by_workflow, {"Loop skill tests": "FAILURE", "LibrAgent CI/CD": "SUCCESS"})

    def test_status_context_uses_context_field(self) -> None:
        checks = [
            run(None, "FAILURE", started="2026-11-27T01:00:00Z", context="lint"),
            run(None, "SUCCESS", started="2026-11-27T01:05:00Z", context="lint"),
        ]
        eff = effective_checks(checks)
        self.assertEqual(len(eff), 1)
        self.assertEqual(eff[0]["context"], "lint")
        self.assertEqual(check_state(eff[0]), "SUCCESS")


if __name__ == "__main__":
    unittest.main()
