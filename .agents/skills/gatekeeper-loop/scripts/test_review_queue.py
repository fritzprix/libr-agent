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
    PROTECTED_SKILL_PREFIXES,
    START_MARKER,
    classify,
    classify_sensitive_paths,
    determine_status,
    is_dev_base,
    is_rebased,
    latest_clean_marker,
    markers_at_head,
    owner_comments_after,
    parse_markers,
    posting_checks,
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

    def test_stacked_pr_reviewed_clean_awaits_ci(self) -> None:
        # Base is a feature branch -> CI never runs, so the PR stays clean until retargeted.
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
        self.assertEqual(status, "await-ci")

    def test_stacked_pr_not_reviewed_yet_is_review(self) -> None:
        self.assertEqual(determine_status(**base_args(base="feat/x", ci_ran=False)), "review")

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

    def test_approved_stacked_without_ci_is_review(self) -> None:
        verdict = {**self.approve, "base": "feat/x"}
        status, reason, _ = classify(**base_args(
            base="feat/x", last_verdict=verdict, total_verdicts=1, ci_ran=False))
        self.assertEqual(status, "review")
        self.assertIn("without green CI", reason)

    def test_approve_with_no_ci_run_is_review(self) -> None:
        status = determine_status(**base_args(last_verdict=self.approve, total_verdicts=1, ci_ran=False))
        self.assertEqual(status, "review")

    def test_approve_with_pending_ci_is_review(self) -> None:
        status = determine_status(**base_args(
            last_verdict=self.approve, total_verdicts=1, pending_checks=["Test"]))
        self.assertEqual(status, "review")

    def test_clean_with_no_ci_run_awaits_ci(self) -> None:
        status, reason, _ = classify(**base_args(clean_head="head1", ci_ran=False))
        self.assertEqual(status, "await-ci")
        self.assertIn("no CI run", reason)

    def test_clean_on_stacked_pr_awaits_ci(self) -> None:
        status, reason, _ = classify(**base_args(base="feat/x", clean_head="head1", ci_ran=False))
        self.assertEqual(status, "await-ci")
        self.assertIn("retargeted", reason)

    def test_clean_with_green_ci_is_review_for_verdict(self) -> None:
        status, reason, _ = classify(**base_args(clean_head="head1"))
        self.assertEqual(status, "review")
        self.assertIn("CI green", reason)

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

    # A PR that had changes requested at older heads and is now clean at this one.
    earlier = {"head": "head0", "verdict": "changes-requested", "base": "dev/0.9.x"}

    def test_clean_after_earlier_verdicts_awaits_pending_ci(self) -> None:
        status, reason, _ = classify(**base_args(
            last_verdict=self.earlier, total_verdicts=4, clean_head="head1",
            pending_checks=["Test"]))
        self.assertEqual(status, "await-ci")
        self.assertIn("waiting for pending CI", reason)

    def test_clean_after_earlier_verdicts_with_green_ci_asks_for_the_verdict(self) -> None:
        status, reason, priority = classify(**base_args(
            last_verdict=self.earlier, total_verdicts=4, clean_head="head1"))
        self.assertEqual(status, "review")
        self.assertIn("issue the verdict", reason)
        self.assertEqual(priority, 1)

    def test_clean_after_earlier_verdicts_with_failing_ci_is_diagnosed(self) -> None:
        status, reason, _ = classify(**base_args(
            last_verdict=self.earlier, total_verdicts=4, clean_head="head1",
            failing_checks=["Test"]))
        self.assertEqual(status, "review")
        self.assertIn("CI failing, diagnose", reason)

    def test_clean_for_an_older_head_after_verdicts_is_a_new_head(self) -> None:
        status, reason, _ = classify(**base_args(
            head="head2", last_verdict=self.earlier, total_verdicts=4, clean_head="head1",
            pending_checks=["Test"]))
        self.assertEqual(status, "review")
        self.assertIn("new head", reason)

    def test_clean_after_earlier_verdicts_still_hits_the_round_limit(self) -> None:
        status, _, _ = classify(**base_args(
            last_verdict=self.earlier, total_verdicts=5, clean_head="head1",
            pending_checks=["Test"]))
        self.assertEqual(status, "escalate")

    def test_approve_in_the_last_round_is_approved_not_escalated(self) -> None:
        status, _, _ = classify(**base_args(
            last_verdict=self.approve, total_verdicts=5))
        self.assertEqual(status, "approved")

    def test_approve_in_the_last_round_still_escalates_after_a_push(self) -> None:
        status, _, _ = classify(**base_args(
            head="head2", last_verdict=self.approve, total_verdicts=5))
        self.assertEqual(status, "escalate")

    def test_max_rounds_override_allows_another_round(self) -> None:
        status, _, _ = classify(**base_args(
            head="head2", last_verdict=self.approve, total_verdicts=5, max_rounds=6))
        self.assertNotEqual(status, "escalate")

    # Changes requested at the current head, then a clean marker at the same head
    # (the worker fixed the PR body, so no new commit).
    requested_here = {"head": "head1", "verdict": "changes-requested", "base": "dev/0.9.x"}
    answered = {"round": "3", "createdAt": "2026-01-01T00:00:00Z"}

    def test_clean_after_changes_requested_at_the_same_head_awaits_pending_ci(self) -> None:
        status, reason, _ = classify(**base_args(
            last_verdict=self.requested_here, total_verdicts=3, worker_response=self.answered,
            clean_head="head1", pending_checks=["Test"]))
        self.assertEqual(status, "await-ci")
        self.assertIn("waiting for pending CI", reason)

    def test_clean_after_changes_requested_at_the_same_head_with_green_ci_asks_for_the_verdict(self) -> None:
        status, reason, _ = classify(**base_args(
            last_verdict=self.requested_here, total_verdicts=3, worker_response=self.answered,
            clean_head="head1"))
        self.assertEqual(status, "review")
        self.assertIn("issue the verdict", reason)

    def test_clean_at_the_same_head_without_a_response_awaits_ci(self) -> None:
        status, _, _ = classify(**base_args(
            last_verdict=self.requested_here, total_verdicts=3, clean_head="head1",
            pending_checks=["Test"]))
        self.assertEqual(status, "await-ci")

    def test_worker_response_without_a_clean_marker_is_still_review(self) -> None:
        status, reason, _ = classify(**base_args(
            last_verdict=self.requested_here, total_verdicts=3, worker_response=self.answered))
        self.assertEqual(status, "review")
        self.assertIn("worker responded", reason)

    def test_state_alone_does_not_override_changes_requested_at_the_head(self) -> None:
        # Only a clean marker on GitHub counts here; state.json can be stale.
        status, _, _ = classify(**base_args(
            last_verdict=self.requested_here, total_verdicts=3,
            state_reviewed_head="head1", state_pending_ci=True, pending_checks=["Test"]))
        self.assertEqual(status, "wait-worker")

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

    def test_verdict_with_code_quality_section_parsed(self) -> None:
        body = """<!-- gatekeeper-loop: round=1 verdict=changes-requested head=deadbeef base=dev/0.9.x -->

## Gatekeeper · round 1 · Changes requested

### Required

- **G1.1** `src/lib.rs:10` — missing edge case.

### Code quality score (informational)

Total **58/100**. Does not affect the verdict above.

- **Correctness incl. boundary conditions (18/30)** — `src/lib.rs:10` — empty input not handled.
"""
        comments = [{"author": {"login": "me"}, "createdAt": "t", "body": body}]
        verdicts, _ = parse_markers(comments, "me")
        self.assertEqual(len(verdicts), 1)
        self.assertEqual(verdicts[0]["verdict"], "changes-requested")
        self.assertEqual(verdicts[0]["round"], "1")
        self.assertEqual(verdicts[0]["head"], "deadbeef")

    def test_protected_skill_prefixes(self) -> None:
        self.assertIn(".agents/skills/gatekeeper-loop/", PROTECTED_SKILL_PREFIXES)
        self.assertIn(".agents/skills/worker-loop/", PROTECTED_SKILL_PREFIXES)

    def test_quoted_markers_ignored(self) -> None:
        comments = [
            {"author": {"login": "me"}, "createdAt": "t",
             "body": "> <!-- gatekeeper-loop: verdict=approve head=abc -->\nQuoting older verdict"}
        ]
        verdicts, responses = parse_markers(comments, "me")
        self.assertEqual(len(verdicts), 0)
        self.assertEqual(len(responses), 0)

    def test_bom_marker_parsed(self) -> None:
        self.assertTrue(START_MARKER.match("\ufeff<!-- gatekeeper-loop: verdict=approve -->"))
        comments = [
            {"author": {"login": "me"}, "createdAt": "t",
             "body": "\ufeff<!-- gatekeeper-loop: verdict=clean head=abc -->"}
        ]
        verdicts, _ = parse_markers(comments, "me")
        self.assertEqual(len(verdicts), 1)
        self.assertEqual(verdicts[0]["verdict"], "clean")

    def test_owner_comment_quoting_marker_is_surfaced(self) -> None:
        comments = [
            {"author": {"login": "me"},
             "body": "> <!-- gatekeeper-loop: verdict=changes-requested -->\nOwner comment quoting feedback",
             "createdAt": "2026-01-02T00:00:00Z"}
        ]
        found = owner_comments_after(comments, "me", "2026-01-01T00:00:00Z")
        self.assertEqual(len(found), 1)

    def test_every_status_has_reason(self) -> None:
        for kwargs in (
            base_args(mergeable="CONFLICTING"),
            base_args(),
            base_args(total_verdicts=5, last_verdict={"head": "x", "verdict": "changes-requested"}),
        ):
            self.assertTrue(classify(**kwargs)[1])


class SecuritySensitiveTests(unittest.TestCase):
    def test_policy_paths_only(self) -> None:
        paths = ["src-tauri/src/secret_fallback.rs", "src/foo.rs"]
        info = classify_sensitive_paths(paths)
        self.assertEqual(info["securitySensitivePolicyPaths"], ["src-tauri/src/secret_fallback.rs"])
        self.assertEqual(info["securitySensitiveBehaviorHints"], [])
        self.assertEqual(info["securitySensitive"], ["src-tauri/src/secret_fallback.rs"])

    def test_behavior_hint_without_policy_match(self) -> None:
        paths = ["src-tauri/src/rust_executor/client.rs", "docs/readme.md"]
        info = classify_sensitive_paths(paths)
        self.assertEqual(info["securitySensitivePolicyPaths"], [])
        self.assertIn("src-tauri/src/rust_executor/client.rs", info["securitySensitiveBehaviorHints"])
        self.assertIn("src-tauri/src/rust_executor/client.rs", info["securitySensitive"])

    def test_combined_dedupes(self) -> None:
        paths = ["src/secret_store.rs", "src/session_api/routes.rs"]
        info = classify_sensitive_paths(paths)
        self.assertEqual(len(info["securitySensitive"]), 2)
        self.assertIn("src/secret_store.rs", info["securitySensitivePolicyPaths"])
        self.assertIn("src/session_api/routes.rs", info["securitySensitiveBehaviorHints"])


class PostingChecksTests(unittest.TestCase):
    def test_markers_at_head_filters(self) -> None:
        markers = [
            {"verdict": "clean", "head": "aaa", "round": None},
            {"verdict": "changes-requested", "head": "bbb", "round": "1"},
            {"verdict": "clean", "head": "bbb", "round": None},
        ]
        at_b = markers_at_head(markers, "bbb")
        self.assertEqual(len(at_b), 2)
        self.assertEqual({m["verdict"] for m in at_b}, {"changes-requested", "clean"})

    def test_posting_checks_exposes_head_markers(self) -> None:
        head = "deadbeef"
        markers = [
            {"verdict": "clean", "head": head, "round": None, "createdAt": "t1"},
            {"verdict": "changes-requested", "head": "other", "round": "1", "createdAt": "t0"},
        ]
        last = {"verdict": "changes-requested", "head": "other", "round": "1"}
        checks = posting_checks(head=head, markers=markers, last_verdict=last)
        self.assertEqual(checks["headSha"], head)
        self.assertEqual(len(checks["markersAtHead"]), 1)
        self.assertIsNone(checks["lastVerdictAtHead"])


class LatestCleanMarkerTests(unittest.TestCase):
    verdict = {"createdAt": "2026-01-01T10:00:00Z"}
    response = {"createdAt": "2026-01-01T11:00:00Z"}

    def test_clean_after_the_response_counts(self) -> None:
        clean = {"createdAt": "2026-01-01T12:00:00Z", "head": "h"}
        self.assertEqual(latest_clean_marker([clean], self.verdict, self.response), clean)

    def test_clean_before_a_later_response_does_not_count(self) -> None:
        clean = {"createdAt": "2026-01-01T10:30:00Z", "head": "h"}
        self.assertIsNone(latest_clean_marker([clean], self.verdict, self.response))

    def test_clean_before_the_last_verdict_does_not_count(self) -> None:
        clean = {"createdAt": "2026-01-01T09:00:00Z", "head": "h"}
        self.assertIsNone(latest_clean_marker([clean], self.verdict, None))

    def test_without_verdicts_the_newest_clean_counts(self) -> None:
        older = {"createdAt": "2026-01-01T09:00:00Z", "head": "a"}
        newer = {"createdAt": "2026-01-01T12:00:00Z", "head": "b"}
        self.assertEqual(latest_clean_marker([older, newer], None, None), newer)


if __name__ == "__main__":
    unittest.main()
