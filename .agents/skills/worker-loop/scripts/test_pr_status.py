#!/usr/bin/env python3
"""Tests for pr_status action selection and gatekeeper verdict parsing.

Run: python3 .agents/skills/worker-loop/scripts/test_pr_status.py
"""
import unittest

import pr_status


class SelectNextActionTest(unittest.TestCase):
    base = dict(
        turn="none",
        conflicting=False, gatekeeper=None,
        has_comments=False, failing=[], pending=[],
        base_drift=False,
    )

    def action(self, **overrides):
        return pr_status.select_next_action(**{**self.base, **overrides})

    def test_priority_order(self):
        everything = dict(
            turn="worker",
            conflicting=True, gatekeeper={"awaitingWorker": True},
            has_comments=True, failing=["ci"], pending=["ci"],
        )
        everything["base_drift"] = True
        expected = [
            ("conflicting", "resolve-conflict"),
            ("base_drift", "rebase-base"),
            ("gatekeeper", "address-gatekeeper"),
            ("has_comments", "triage-comments"),
            ("failing", "fix-ci"),
            ("pending", "wait-ci"),
        ]
        for key, action in expected:
            self.assertEqual(self.action(**everything), action)
            everything[key] = self.base[key]
        self.assertEqual(self.action(**everything), "none")

    def test_gatekeeper_verdict_that_does_not_await_the_worker_is_not_an_action(self):
        settled = {"verdict": "approve", "awaitingWorker": False}
        self.assertEqual(self.action(gatekeeper=settled), "none")
        self.assertEqual(self.action(gatekeeper=settled, pending=["ci"]), "wait-ci")

    def test_a_clean_pr_needs_no_action(self):
        self.assertEqual(self.action(), "none")

    def test_gatekeeper_turn_blocks_issue_work(self):
        self.assertEqual(
            self.action(turn="gatekeeper"),
            "wait-gatekeeper",
        )

    def test_stale_verdict_on_new_head_waits_for_gatekeeper(self):
        gk = {
            "verdict": "changes-requested",
            "head": "sha1",
            "awaitingWorker": False,
        }
        self.assertEqual(
            self.action(turn="gatekeeper", gatekeeper=gk),
            "wait-gatekeeper",
        )


class BaseDriftTest(unittest.TestCase):
    def drift(self, merge_state="CLEAN", ahead=0, base_files=(), pr_files=("src/a.rs",)):
        return pr_status.base_drift_needs_rebase(
            merge_state=merge_state, base_ahead_by=ahead, base_files=base_files, pr_files=pr_files,
        )

    def test_up_to_date_pr_is_not_rebased(self):
        self.assertFalse(self.drift(merge_state="CLEAN", ahead=0))

    def test_behind_below_threshold_on_unrelated_files_is_not_rebased(self):
        threshold = pr_status.BASE_DRIFT_COMMITS
        self.assertFalse(self.drift(merge_state="UNSTABLE", ahead=threshold - 1, base_files=["docs/x.md"]))

    def test_behind_at_or_above_threshold_is_rebased(self):
        threshold = pr_status.BASE_DRIFT_COMMITS
        self.assertTrue(self.drift(merge_state="UNSTABLE", ahead=threshold))
        self.assertTrue(self.drift(merge_state="CLEAN", ahead=threshold + 5))

    def test_base_commit_touching_a_pr_file_is_rebased(self):
        self.assertTrue(self.drift(ahead=1, base_files=["docs/x.md", "src/a.rs"]))

    def test_github_behind_state_is_rebased(self):
        self.assertTrue(self.drift(merge_state="BEHIND", ahead=1, base_files=["docs/x.md"]))


class MergeableUnknownTest(unittest.TestCase):
    def run_fetch(self, states):
        replies = iter({"mergeable": s} for s in states)
        sleeps = []
        pr = pr_status.fetch_until_mergeable_known(lambda: next(replies), sleeps.append, delays=(1, 2, 3))
        return pr["mergeable"], sleeps

    def test_known_state_is_not_refetched(self):
        self.assertEqual(self.run_fetch(["MERGEABLE"]), ("MERGEABLE", []))

    def test_unknown_is_refetched_until_known(self):
        self.assertEqual(self.run_fetch(["UNKNOWN", "UNKNOWN", "CONFLICTING"]), ("CONFLICTING", [1, 2]))

    def test_unknown_after_all_retries_stays_unknown(self):
        self.assertEqual(self.run_fetch(["UNKNOWN"] * 4), ("UNKNOWN", [1, 2, 3]))


class MarkerAndGatekeeperStateTest(unittest.TestCase):
    def test_marker_must_be_at_comment_start(self):
        self.assertTrue(pr_status.START_MARKER.match("<!-- gatekeeper-loop: verdict=approve -->"))
        self.assertTrue(pr_status.START_MARKER.match("\n  <!-- gatekeeper-loop: verdict=approve -->"))
        # Leading BOM support
        self.assertTrue(pr_status.START_MARKER.match("\ufeff<!-- gatekeeper-loop: verdict=approve -->"))
        self.assertTrue(pr_status.START_MARKER.match("\ufeff  \n<!-- gatekeeper-loop: verdict=approve -->"))
        # Quoted marker must not match
        self.assertIsNone(pr_status.START_MARKER.match("> <!-- gatekeeper-loop: verdict=approve -->"))
        # Marker later in body must not match
        self.assertIsNone(pr_status.START_MARKER.match("Previous verdict:\n<!-- gatekeeper-loop: verdict=approve -->"))

    def test_gatekeeper_state_ignores_quoted_verdicts(self):
        comments = [
            {
                "author": {"login": "me"},
                "createdAt": "2026-10-31T08:00:00Z",
                "body": "> <!-- gatekeeper-loop: verdict=changes-requested round=1 head=sha1 -->\nquoted text",
                "url": "http://example.com/c1",
            }
        ]
        self.assertIsNone(pr_status.gatekeeper_state(comments, "me", "sha1"))

    def test_gatekeeper_state_valid_verdict(self):
        comments = [
            {
                "author": {"login": "me"},
                "createdAt": "2026-10-31T08:00:00Z",
                "body": "<!-- gatekeeper-loop: verdict=changes-requested round=1 head=sha1 -->\nReal verdict",
                "url": "http://example.com/c1",
            }
        ]
        state = pr_status.gatekeeper_state(comments, "me", "sha1")
        self.assertIsNotNone(state)
        self.assertEqual(state["verdict"], "changes-requested")
        self.assertTrue(state["awaitingWorker"])

    def test_stale_verdict_on_an_older_head_is_settled(self):
        comments = [
            {
                "author": {"login": "me"},
                "createdAt": "2026-10-31T08:00:00Z",
                "body": "<!-- gatekeeper-loop: verdict=changes-requested round=1 head=sha1 -->",
                "url": "http://example.com/c1",
            }
        ]
        state = pr_status.gatekeeper_state(comments, "me", "sha2")
        self.assertFalse(state["awaitingWorker"])

    def test_worker_response_settles_the_round(self):
        comments = [
            {
                "author": {"login": "me"},
                "createdAt": "2026-10-31T08:00:00Z",
                "body": "<!-- gatekeeper-loop: verdict=changes-requested round=1 head=sha1 -->",
                "url": "http://example.com/c1",
            },
            {
                "author": {"login": "me"},
                "createdAt": "2026-10-31T09:00:00Z",
                "body": "<!-- worker-loop: response round=1 head=sha2 -->",
                "url": "http://example.com/c2",
            },
        ]
        state = pr_status.gatekeeper_state(comments, "me", "sha1")
        self.assertFalse(state["awaitingWorker"])

    def test_needs_owner_comment_does_not_settle_a_round(self):
        comments = [
            {
                "author": {"login": "me"},
                "createdAt": "2026-10-31T08:00:00Z",
                "body": "<!-- gatekeeper-loop: verdict=changes-requested round=1 head=sha1 -->",
            },
            {
                "author": {"login": "me"},
                "createdAt": "2026-10-31T09:00:00Z",
                "body": "<!-- worker-loop: needs-owner -->\n(a) or (b)?",
            },
        ]
        self.assertTrue(pr_status.gatekeeper_state(comments, "me", "sha1")["awaitingWorker"])


class OwnerQuestionAndCiAttemptsTest(unittest.TestCase):
    def test_waiting_on_the_owner_comes_first(self):
        found = pr_status.select_next_action(
            turn="worker", conflicting=True, gatekeeper={"awaitingWorker": True},
            has_comments=True, failing=["ci"], pending=[], waiting_owner=True,
        )
        self.assertEqual(found, "wait-owner")

    def test_ci_attempts_count_only_own_markers(self):
        comments = [
            {"author": {"login": "me"}, "createdAt": "1", "body": "<!-- worker-loop: ci-attempt n=1 -->"},
            {"author": {"login": "me"}, "createdAt": "2", "body": "<!-- worker-loop: response round=1 -->"},
            {"author": {"login": "x"}, "createdAt": "3", "body": "<!-- worker-loop: ci-attempt n=2 -->"},
            {"author": {"login": "me"}, "createdAt": "4", "body": "> <!-- worker-loop: ci-attempt n=2 -->"},
            {"author": {"login": "me"}, "createdAt": "5", "body": "<!-- worker-loop: ci-attempt n=2 -->"},
        ]
        self.assertEqual(pr_status.ci_attempts(comments, "me"), 2)


if __name__ == "__main__":
    unittest.main()
