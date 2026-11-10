#!/usr/bin/env python3
"""Tests for pr_status action selection and gatekeeper verdict parsing.

Run: python3 .agents/skills/worker-loop/scripts/test_pr_status.py
"""
import unittest

import pr_status


class SelectNextActionTest(unittest.TestCase):
    base = dict(
        conflicting=False, gatekeeper=None,
        has_comments=False, failing=[], pending=[],
    )

    def action(self, **overrides):
        return pr_status.select_next_action(**{**self.base, **overrides})

    def test_priority_order(self):
        everything = dict(
            conflicting=True, gatekeeper={"awaitingWorker": True},
            has_comments=True, failing=["ci"], pending=["ci"],
        )
        expected = [
            ("conflicting", "resolve-conflict"),
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


if __name__ == "__main__":
    unittest.main()
