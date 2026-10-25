#!/usr/bin/env python3
"""Tests for gatekeeper-loop review_queue status determination."""
from __future__ import annotations

import unittest
from pathlib import Path
import sys

SCRIPTS_DIR = Path(__file__).resolve().parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from review_queue import (
    classify,
    determine_status,
    is_dev_base,
    is_rebased,
    owner_comments_after,
    parse_markers,
)


def base_args(**over):
    args = dict(
        mergeable="MERGEABLE", head="head1", base="dev/0.9.x", last_verdict=None,
        worker_response=None, total_verdicts=0, state_reviewed_head=None,
        state_pending_ci=False, has_open_findings=False, pending_checks=[], failing_checks=[],
    )
    args.update(over)
    return args


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


class GapTests(unittest.TestCase):
    approve = {"head": "head1", "verdict": "approve", "base": "dev/0.9.x"}

    def test_approved_with_ci_failure_is_review(self) -> None:
        status, reason, _ = classify(**base_args(
            last_verdict=self.approve, total_verdicts=1, failing_checks=["Test"]))
        self.assertEqual(status, "review")
        self.assertIn("CI failing", reason)

    def test_approved_stacked_ignores_ci(self) -> None:
        verdict = {**self.approve, "base": "feat/x"}
        status = determine_status(**base_args(
            base="feat/x", last_verdict=verdict, total_verdicts=1, failing_checks=["Test"]))
        self.assertEqual(status, "approved")

    def test_base_change_after_approve_is_review(self) -> None:
        verdict = {**self.approve, "base": "feat/x"}
        status, reason, _ = classify(**base_args(
            base="dev/0.9.x", last_verdict=verdict, total_verdicts=1, reviewed_base="feat/x"))
        self.assertEqual(status, "review")
        self.assertIn("base changed", reason)

    def test_missing_state_with_clean_marker_is_await_ci(self) -> None:
        status = determine_status(**base_args(
            clean_head="head1", state_reviewed_head=None, pending_checks=["Test"]))
        self.assertEqual(status, "await-ci")

    def test_clean_marker_for_old_head_is_review(self) -> None:
        status = determine_status(**base_args(clean_head="head0", pending_checks=["Test"]))
        self.assertEqual(status, "review")

    def test_clean_marker_with_green_ci_is_review(self) -> None:
        status = determine_status(**base_args(clean_head="head1"))
        self.assertEqual(status, "review")

    def test_rebased_head_is_review(self) -> None:
        last = {"head": "old", "verdict": "changes-requested"}
        status, reason, _ = classify(**base_args(
            last_verdict=last, total_verdicts=1, worker_response={"head": "head1"}, rebased=True))
        self.assertEqual(status, "review")
        self.assertIn("rebased", reason)

    def test_is_rebased(self) -> None:
        self.assertFalse(is_rebased(None, "h", lambda a, b: True))
        self.assertFalse(is_rebased("h", "h", lambda a, b: False))
        self.assertFalse(is_rebased("old", "new", lambda a, b: True))
        self.assertTrue(is_rebased("old", "new", lambda a, b: False))

    def test_owner_comment_after_verdict_is_review(self) -> None:
        last = {"head": "head1", "verdict": "changes-requested"}
        status, _, priority = classify(**base_args(
            last_verdict=last, total_verdicts=1, owner_comment=True))
        self.assertEqual(status, "review")
        self.assertEqual(priority, 0)

    def test_owner_comment_beats_approved(self) -> None:
        status = determine_status(**base_args(
            last_verdict=self.approve, total_verdicts=1, owner_comment=True))
        self.assertEqual(status, "review")

    def test_owner_comments_after_filters(self) -> None:
        comments = [
            {"author": {"login": "me"}, "body": "<!-- gatekeeper-loop: round=1 verdict=clean head=a -->",
             "createdAt": "2026-01-01T00:00:00Z"},
            {"author": {"login": "me"}, "body": "README에 secret 언급 추가", "createdAt": "2026-01-02T00:00:00Z"},
            {"author": {"login": "bot[bot]"}, "body": "ci", "createdAt": "2026-01-02T00:00:00Z"},
            {"author": {"login": "rando"}, "body": "hi", "createdAt": "2026-01-02T00:00:00Z",
             "authorAssociation": "NONE"},
            {"author": {"login": "me"}, "body": "old note", "createdAt": "2025-12-31T00:00:00Z"},
        ]
        found = owner_comments_after(comments, "me", "2026-01-01T00:00:00Z")
        self.assertEqual([c["body"] for c in found], ["README에 secret 언급 추가"])

    def test_clean_marker_parsed(self) -> None:
        comments = [{"author": {"login": "me"}, "createdAt": "t",
                     "body": "<!-- gatekeeper-loop: verdict=clean head=abc -->"}]
        verdicts, _ = parse_markers(comments, "me")
        self.assertEqual(verdicts[0]["verdict"], "clean")
        self.assertEqual(verdicts[0]["head"], "abc")

    def test_every_status_has_reason(self) -> None:
        for kwargs in (
            base_args(mergeable="CONFLICTING"),
            base_args(),
            base_args(total_verdicts=5, last_verdict={"head": "x", "verdict": "changes-requested"}),
        ):
            self.assertTrue(classify(**kwargs)[1])


if __name__ == "__main__":
    unittest.main()
