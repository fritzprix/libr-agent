#!/usr/bin/env python3
"""Tests for latest-workflow-run CI selection.

Run: python3 .agents/skills/loop-shared/scripts/test_ci_checks.py
"""
from __future__ import annotations

import unittest
from pathlib import Path
import sys

SCRIPTS = Path(__file__).resolve().parent
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from ci_checks import (  # noqa: E402
    check_run_ts,
    check_state,
    effective_checks,
    workflow_run_id,
)


def run(
    name,
    conclusion="",
    status="",
    started=None,
    created=None,
    completed=None,
    workflow="LibrAgent CI/CD",
    context=None,
    run_id=None,
    typ=None,
):
    item = {}
    if typ:
        item["__typename"] = typ
    elif context is not None and name is None:
        item["__typename"] = "StatusContext"
    else:
        item["__typename"] = "CheckRun"
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
    if run_id is not None:
        item["detailsUrl"] = (
            f"https://github.com/fritzprix/libr-agent/actions/runs/{run_id}/job/1"
        )
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


class WorkflowRunIdTests(unittest.TestCase):
    def test_reads_details_url_and_check_suite(self) -> None:
        self.assertEqual(
            workflow_run_id({
                "detailsUrl": "https://github.com/fritzprix/libr-agent/actions/runs/45956271742/job/1",
            }),
            "45956271742",
        )
        self.assertEqual(workflow_run_id({"checkSuite": {"id": 99}}), "99")
        self.assertIsNone(workflow_run_id({"name": "Test"}))


class EffectiveChecksTests(unittest.TestCase):
    def test_older_run_cancelled_or_failed_then_newer_run_green(self) -> None:
        checks = [
            run("Test", "CANCELLED", started="2026-11-27T01:00:00Z", completed="2026-11-27T01:01:00Z", run_id=1),
            run("Build", "FAILURE", started="2026-11-27T01:00:00Z", completed="2026-11-27T01:03:00Z", run_id=1),
            run("Test", "SUCCESS", started="2026-11-27T01:05:00Z", completed="2026-11-27T01:07:00Z", run_id=2),
            run("Build", "SUCCESS", started="2026-11-27T01:05:00Z", completed="2026-11-27T01:08:00Z", run_id=2),
        ]
        eff = effective_checks(checks)
        self.assertEqual(
            [(c["name"], check_state(c)) for c in eff],
            [("Test", "SUCCESS"), ("Build", "SUCCESS")],
        )

    def test_newer_run_in_progress_is_pending(self) -> None:
        checks = [
            run("Test", "SUCCESS", started="2026-11-27T01:00:00Z", completed="2026-11-27T01:02:00Z", run_id=1),
            run(
                "Test",
                status="IN_PROGRESS",
                started="2026-11-27T01:05:00Z",
                completed="0001-01-01T00:00:00Z",
                run_id=2,
            ),
        ]
        eff = effective_checks(checks)
        self.assertEqual(len(eff), 1)
        self.assertEqual(check_state(eff[0]), "IN_PROGRESS")

    def test_matrix_parent_cancelled_in_old_run_does_not_hide_new_run(self) -> None:
        checks = [
            run("Resolve OS matrix", "CANCELLED", started="2026-11-27T06:51:30Z", run_id=10),
            run("Test", "CANCELLED", started="2026-11-27T07:07:12Z", run_id=10),
            run("Build", "CANCELLED", started="2026-11-27T07:07:12Z", run_id=10),
            run("Resolve OS matrix", "SUCCESS", started="2026-11-27T07:09:32Z", run_id=11),
            run("Test (ubuntu-latest)", "SUCCESS", started="2026-11-27T07:28:38Z", run_id=11),
            run("Test (windows-latest)", "SUCCESS", started="2026-11-27T07:09:47Z", run_id=11),
            run("Test (macos-15)", "SUCCESS", started="2026-11-27T07:30:01Z", run_id=11),
            run("Build (ubuntu-latest)", "SUCCESS", started="2026-11-27T08:09:11Z", run_id=11),
        ]
        eff = effective_checks(checks)
        names = [c["name"] for c in eff]
        self.assertNotIn("Test", names)
        self.assertNotIn("Build", names)
        self.assertEqual(
            [(c["name"], check_state(c)) for c in eff],
            [
                ("Resolve OS matrix", "SUCCESS"),
                ("Test (ubuntu-latest)", "SUCCESS"),
                ("Test (windows-latest)", "SUCCESS"),
                ("Test (macos-15)", "SUCCESS"),
                ("Build (ubuntu-latest)", "SUCCESS"),
            ],
        )

    def test_all_skipped_newer_run_is_not_a_result(self) -> None:
        checks = [
            run("Test (ubuntu-latest)", "SUCCESS", started="2026-11-27T01:51:53Z", run_id=20),
            run("Test (macos-15)", "SUCCESS", started="2026-11-27T03:58:21Z", run_id=20),
            run("Build (macos-15)", "SUCCESS", started="2026-11-27T05:25:24Z", run_id=20),
            run("Resolve OS matrix", "SKIPPED", started="2026-11-27T06:30:17Z", run_id=21),
            run("Test", "SKIPPED", started="2026-11-27T06:30:17Z", run_id=21),
            run("Build", "SKIPPED", started="2026-11-27T06:30:17Z", run_id=21),
        ]
        eff = effective_checks(checks)
        self.assertEqual(
            [(c["name"], check_state(c)) for c in eff],
            [
                ("Test (ubuntu-latest)", "SUCCESS"),
                ("Test (macos-15)", "SUCCESS"),
                ("Build (macos-15)", "SUCCESS"),
            ],
        )

    def test_same_name_in_two_workflows_is_independent(self) -> None:
        checks = [
            run("test", "FAILURE", started="2026-11-27T01:00:00Z", workflow="Loop skill tests", run_id=30),
            run("test", "SUCCESS", started="2026-11-27T01:00:00Z", workflow="LibrAgent CI/CD", run_id=31),
            run("test", "CANCELLED", started="2026-11-27T00:50:00Z", workflow="LibrAgent CI/CD", run_id=30),
        ]
        eff = effective_checks(checks)
        by_workflow = {c["workflowName"]: check_state(c) for c in eff}
        self.assertEqual(by_workflow, {"Loop skill tests": "FAILURE", "LibrAgent CI/CD": "SUCCESS"})

    def test_status_context_keeps_latest_per_context(self) -> None:
        checks = [
            run(None, "FAILURE", started="2026-11-27T01:00:00Z", context="lint", workflow=None),
            run(None, "SUCCESS", started="2026-11-27T01:05:00Z", context="lint", workflow=None),
        ]
        eff = effective_checks(checks)
        self.assertEqual(len(eff), 1)
        self.assertEqual(eff[0]["context"], "lint")
        self.assertEqual(check_state(eff[0]), "SUCCESS")


if __name__ == "__main__":
    unittest.main()
