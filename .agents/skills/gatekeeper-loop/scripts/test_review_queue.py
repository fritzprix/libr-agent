#!/usr/bin/env python3
"""Tests for gatekeeper-loop review_queue status determination."""
from __future__ import annotations

import unittest
from pathlib import Path
import sys

SCRIPTS_DIR = Path(__file__).resolve().parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from review_queue import determine_status, is_dev_base


class DetermineStatusTests(unittest.TestCase):
    def test_conflict_is_wait_worker(self) -> None:
        status = determine_status(
            mergeable="CONFLICTING",
            head="head1",
            base="dev/0.9.x",
            last_verdict=None,
            worker_response=None,
            total_verdicts=0,
            state_reviewed_head=None,
            state_pending_ci=False,
            has_open_findings=False,
            pending_checks=[],
            failing_checks=[],
        )
        self.assertEqual(status, "wait-worker")

    def test_approved_at_current_head(self) -> None:
        status = determine_status(
            mergeable="MERGEABLE",
            head="head1",
            base="dev/0.9.x",
            last_verdict={"head": "head1", "verdict": "approve"},
            worker_response=None,
            total_verdicts=1,
            state_reviewed_head="head1",
            state_pending_ci=False,
            has_open_findings=False,
            pending_checks=[],
            failing_checks=[],
        )
        self.assertEqual(status, "approved")

    def test_changes_requested_without_response_is_wait_worker(self) -> None:
        status = determine_status(
            mergeable="MERGEABLE",
            head="head1",
            base="dev/0.9.x",
            last_verdict={"head": "head1", "verdict": "changes-requested"},
            worker_response=None,
            total_verdicts=1,
            state_reviewed_head="head1",
            state_pending_ci=False,
            has_open_findings=True,
            pending_checks=[],
            failing_checks=[],
        )
        self.assertEqual(status, "wait-worker")

    def test_changes_requested_with_response_is_review(self) -> None:
        status = determine_status(
            mergeable="MERGEABLE",
            head="head1",
            base="dev/0.9.x",
            last_verdict={"head": "head1", "verdict": "changes-requested"},
            worker_response={"head": "head1"},
            total_verdicts=1,
            state_reviewed_head="head1",
            state_pending_ci=False,
            has_open_findings=True,
            pending_checks=["Test"],
            failing_checks=[],
        )
        self.assertEqual(status, "review")

    def test_new_head_after_verdict_is_review(self) -> None:
        status = determine_status(
            mergeable="MERGEABLE",
            head="head2",
            base="dev/0.9.x",
            last_verdict={"head": "head1", "verdict": "changes-requested"},
            worker_response=None,
            total_verdicts=1,
            state_reviewed_head="head1",
            state_pending_ci=False,
            has_open_findings=True,
            pending_checks=["Test"],
            failing_checks=[],
        )
        self.assertEqual(status, "review")

    def test_unreviewed_head_with_failing_ci_is_review(self) -> None:
        # Crucial fix: failing CI on an unreviewed head must be reviewed, NOT skipped to wait-worker.
        status = determine_status(
            mergeable="MERGEABLE",
            head="head1",
            base="dev/0.9.x",
            last_verdict=None,
            worker_response=None,
            total_verdicts=0,
            state_reviewed_head=None,
            state_pending_ci=False,
            has_open_findings=False,
            pending_checks=[],
            failing_checks=["Build (ubuntu-latest)"],
        )
        self.assertEqual(status, "review")

    def test_reviewed_head_waiting_ci_is_await_ci(self) -> None:
        # Head was already reviewed with 0 findings; waiting on CI.
        status = determine_status(
            mergeable="MERGEABLE",
            head="head1",
            base="dev/0.9.x",
            last_verdict=None,
            worker_response=None,
            total_verdicts=0,
            state_reviewed_head="head1",
            state_pending_ci=True,
            has_open_findings=False,
            pending_checks=["Test (ubuntu-latest)"],
            failing_checks=[],
        )
        self.assertEqual(status, "await-ci")

    def test_await_ci_turns_to_review_when_ci_green(self) -> None:
        # When pending checks clear with 0 failures, enters review to post approve verdict.
        status = determine_status(
            mergeable="MERGEABLE",
            head="head1",
            base="dev/0.9.x",
            last_verdict=None,
            worker_response=None,
            total_verdicts=0,
            state_reviewed_head="head1",
            state_pending_ci=True,
            has_open_findings=False,
            pending_checks=[],
            failing_checks=[],
        )
        self.assertEqual(status, "review")

    def test_await_ci_turns_to_review_when_ci_fails(self) -> None:
        # When a pending check fails, enters review so gatekeeper diagnoses logs and requests changes.
        status = determine_status(
            mergeable="MERGEABLE",
            head="head1",
            base="dev/0.9.x",
            last_verdict=None,
            worker_response=None,
            total_verdicts=0,
            state_reviewed_head="head1",
            state_pending_ci=True,
            has_open_findings=False,
            pending_checks=[],
            failing_checks=["Test (macos-15)"],
        )
        self.assertEqual(status, "review")

    def test_new_head_pushed_while_await_ci_is_review(self) -> None:
        # New commit pushed -> immediately review diff, do not stay in await-ci.
        status = determine_status(
            mergeable="MERGEABLE",
            head="head2",
            base="dev/0.9.x",
            last_verdict=None,
            worker_response=None,
            total_verdicts=0,
            state_reviewed_head="head1",
            state_pending_ci=True,
            has_open_findings=False,
            pending_checks=["Test"],
            failing_checks=[],
        )
        self.assertEqual(status, "review")

    def test_stacked_pr_does_not_await_ci(self) -> None:
        # Base is a feature branch -> CI does not trigger, never block on await-ci.
        status = determine_status(
            mergeable="MERGEABLE",
            head="head1",
            base="fix/2242-docker-skill-roots",
            last_verdict=None,
            worker_response=None,
            total_verdicts=0,
            state_reviewed_head="head1",
            state_pending_ci=True,
            has_open_findings=False,
            pending_checks=["Any"],
            failing_checks=[],
        )
        self.assertEqual(status, "review")

    def test_max_rounds_escalates(self) -> None:
        status = determine_status(
            mergeable="MERGEABLE",
            head="head6",
            base="dev/0.9.x",
            last_verdict={"head": "head5", "verdict": "changes-requested"},
            worker_response=None,
            total_verdicts=5,
            state_reviewed_head="head5",
            state_pending_ci=False,
            has_open_findings=False,
            pending_checks=[],
            failing_checks=[],
        )
        self.assertEqual(status, "escalate")

    def test_is_dev_base(self) -> None:
        self.assertTrue(is_dev_base("dev/0.9.x"))
        self.assertTrue(is_dev_base("main"))
        self.assertTrue(is_dev_base("master"))
        self.assertFalse(is_dev_base("fix/2242-docker-skill-roots"))
        self.assertFalse(is_dev_base("feat/something"))


if __name__ == "__main__":
    unittest.main()
