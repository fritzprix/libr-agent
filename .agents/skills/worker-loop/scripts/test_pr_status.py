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
        blockers=None, rebase_ready=False,
    )

    def action(self, **overrides):
        return pr_status.select_next_action(**{**self.base, **overrides})

    def test_priority_order(self):
        everything = dict(
            turn="worker",
            conflicting=True, gatekeeper={"awaitingWorker": True},
            has_comments=True, failing=["ci"], pending=["ci"],
            blockers=[9], rebase_ready=True,
        )
        expected = [
            ("blockers", "wait-blocker"),
            ("conflicting", "resolve-conflict"),
            ("rebase_ready", "rebase"),
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


class WaitBlockerTest(unittest.TestCase):
    def test_open_blocker_waits(self):
        self.assertEqual(self.action(blockers=[9]), "wait-blocker")

    def test_no_blockers_is_not_wait_blocker(self):
        self.assertEqual(self.action(blockers=[]), "none")

    def test_wait_owner_outranks_wait_blocker(self):
        self.assertEqual(
            self.action(blockers=[9], waiting_owner=True),
            "wait-owner",
        )

    def action(self, **overrides):
        return pr_status.select_next_action(
            turn="worker", conflicting=False, gatekeeper=None,
            has_comments=False, failing=[], pending=[], **overrides,
        )


class RebaseReadyTest(unittest.TestCase):
    CLEAN = "2026-11-25T00:00:00Z"

    def marker(self, verdict, head, at=CLEAN):
        return {"author": {"login": "me"}, "createdAt": at,
                "body": f"<!-- gatekeeper-loop: verdict={verdict} head={head} -->",
                "url": f"http://example.com/{verdict}-{head}"}

    def comments(self, *bodies_verdicts_heads):
        return [self.marker(v, h, f"2026-11-25T0{i}:00:00Z")
                for i, (v, h) in enumerate(bodies_verdicts_heads, start=1)]

    def blockers(self, *states):
        return [{"number": i + 1, "state": s} for i, s in enumerate(states)]

    def test_all_closed_blockers_with_clean_at_head_is_ready(self):
        comments = self.comments(("clean", "sha1"))
        self.assertTrue(pr_status.rebase_ready(self.blockers("closed"), comments, "me", "sha1"))

    def test_pr_without_blocker_link_is_never_ready(self):
        comments = self.comments(("clean", "sha1"))
        self.assertFalse(pr_status.rebase_ready([], comments, "me", "sha1"))

    def test_open_blocker_is_not_ready(self):
        comments = self.comments(("clean", "sha1"))
        self.assertFalse(pr_status.rebase_ready(self.blockers("closed", "open"), comments, "me", "sha1"))

    def test_latest_marker_must_be_clean_at_head(self):
        comments = self.comments(("clean", "sha1"), ("changes-requested", "sha1"))
        self.assertFalse(pr_status.rebase_ready(self.blockers("closed"), comments, "me", "sha1"))

    def test_clean_at_older_head_is_not_ready(self):
        comments = self.comments(("clean", "sha0"))
        self.assertFalse(pr_status.rebase_ready(self.blockers("closed"), comments, "me", "sha1"))

    def test_no_verdict_at_all_is_not_ready(self):
        self.assertFalse(pr_status.rebase_ready(self.blockers("closed"), [], "me", "sha1"))


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

    def test_unknown_gatekeeper_verdict_is_ignored(self):
        """Issue #2994: a verdict outside the fixed list is not a verdict."""
        comments = [
            {
                "author": {"login": "me"},
                "createdAt": "2026-10-31T08:00:00Z",
                "body": "<!-- gatekeeper-loop: verdict=ci-report round=1 head=sha1 -->",
                "url": "http://example.com/c1",
            }
        ]
        self.assertIsNone(pr_status.gatekeeper_state(comments, "me", "sha1"))

    def test_clean_marker_accepts_no_blocked_by_field(self):
        comments = [
            {
                "author": {"login": "me"},
                "createdAt": "2026-10-31T08:00:00Z",
                "body": "<!-- gatekeeper-loop: verdict=clean head=sha1 blocked-by=2995 -->",
                "url": "http://example.com/c1",
            }
        ]
        state = pr_status.gatekeeper_state(comments, "me", "sha1")
        self.assertEqual(state["verdict"], "clean")
        self.assertNotIn("blocked-by", state)

    def test_non_response_markers_with_round_do_not_settle_the_round(self):
        """G1.1: unknown kinds, `reply`, and `ci-attempt` never settle a round;
        the next action stays `address-gatekeeper`."""
        verdict = {
            "author": {"login": "me"},
            "createdAt": "2026-10-31T08:00:00Z",
            "body": "<!-- gatekeeper-loop: verdict=changes-requested round=1 head=sha1 -->",
            "url": "http://example.com/c1",
        }
        for body, created in (
            ("<!-- worker-loop: ci-report round=1 head=sha1 -->", "2026-10-31T09:00:00Z"),
            ("<!-- worker-loop: reply reply-to=42 round=1 -->", "2026-10-31T09:10:00Z"),
            ("<!-- worker-loop: ci-attempt round=1 n=1 -->", "2026-10-31T09:20:00Z"),
        ):
            comments = [verdict, {"author": {"login": "me"}, "createdAt": created,
                                  "body": body, "url": "http://example.com/c2"}]
            state = pr_status.gatekeeper_state(comments, "me", "sha1")
            self.assertTrue(state["awaitingWorker"], body)
            self.assertEqual(
                pr_status.select_next_action(
                    turn="worker", conflicting=False, gatekeeper=state,
                    has_comments=False, failing=[], pending=[],
                ),
                "address-gatekeeper",
                body,
            )

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


class NewTriageCommentsTest(unittest.TestCase):
    """Issue #2994: a comment answered by a worker-loop `reply` is settled.

    Ids are strings like GitHub's comment ids, and the replied set comes from
    `reply_targets` so the composition at pr_status.summarize is covered.
    """

    COMMENTS = [
        {"id": "101", "author": {"login": "owner"}, "createdAt": "2026-11-01T10:00:00Z",
         "body": "Please check the timeout.", "url": "http://example.com/c101"},
        {"id": "102", "author": {"login": "owner"}, "createdAt": "2026-11-01T11:00:00Z",
         "body": "And the retry count too.", "url": "http://example.com/c102"},
        {"id": "103", "author": {"login": "me"}, "createdAt": "2026-11-01T12:00:00Z",
         "body": "<!-- worker-loop: reply reply-to=101 -->\nChecked.", "url": "http://example.com/c103"},
    ]

    def test_unanswered_comments_stay_triage_items(self):
        replied = pr_status.reply_targets(self.COMMENTS, "me")
        items = pr_status.new_triage_comments(self.COMMENTS, "2026-11-01T09:00:00Z", replied)
        self.assertEqual([item["id"] for item in items], ["102"])

    def test_without_reply_markers_everything_after_the_commit_is_a_triage_item(self):
        items = pr_status.new_triage_comments(self.COMMENTS, "2026-11-01T09:00:00Z", set())
        # The reply marker comment itself is marker-prefixed, so never a triage item.
        self.assertEqual([item["id"] for item in items], ["101", "102"])


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

    def test_behind_triggers_rebase_and_base_drift_triggers_rebase_base(self):
        behind_action = pr_status.select_next_action(
            turn="gatekeeper", conflicting=False, gatekeeper=None,
            has_comments=False, failing=[], pending=[], behind=True,
        )
        self.assertEqual(behind_action, "rebase")

        drift_action = pr_status.select_next_action(
            turn="gatekeeper", conflicting=False, gatekeeper=None,
            has_comments=False, failing=[], pending=[], base_drift=True,
        )
        self.assertEqual(drift_action, "rebase-base")

        self.assertTrue(pr_status.base_drift_needs_rebase(
            merge_state="BEHIND", base_ahead_by=1, base_files=[], pr_files=[],
        ))


if __name__ == "__main__":
    unittest.main()
