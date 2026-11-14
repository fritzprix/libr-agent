#!/usr/bin/env python3
"""Tests for shared PR turn ownership."""
from __future__ import annotations

import unittest
from pathlib import Path
import sys

SCRIPTS = Path(__file__).resolve().parent
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from pr_turn import decide_pr_turn, label_mismatches  # noqa: E402


def gk(verdict: str, head: str, round_: str = "1", created: str = "t1") -> dict:
    return {
        "author": {"login": "me"},
        "createdAt": created,
        "body": f"<!-- gatekeeper-loop: verdict={verdict} round={round_} head={head} -->",
    }


def wr(round_: str, head: str, created: str = "t2") -> dict:
    return {
        "author": {"login": "me"},
        "createdAt": created,
        "body": f"<!-- worker-loop: response round={round_} head={head} -->",
    }


class PrTurnTests(unittest.TestCase):
    def test_new_head_after_changes_requested_is_gatekeeper(self) -> None:
        """Regression for #2737: worker fixed on new head, gatekeeper must re-review."""
        turn = decide_pr_turn(
            head_sha="sha2",
            mergeable="MERGEABLE",
            comments=[
                gk("changes-requested", "sha1"),
                wr("1", "sha2", "t2"),
            ],
            me="me",
        )
        self.assertEqual(turn.turn, "gatekeeper")
        self.assertEqual(turn.currentVerdict, "changes-requested")
        self.assertEqual(turn.verdictHead, "sha1")
        self.assertFalse(turn.atVerdictHead)

    def test_clean_at_current_head_supersedes_older_changes_requested(self) -> None:
        """G1.4: clean at the current head IS the current verdict, not the older changes-requested."""
        turn = decide_pr_turn(
            head_sha="sha2",
            mergeable="MERGEABLE",
            comments=[
                gk("changes-requested", "sha1", round_="1", created="t1"),
                gk("clean", "sha2", round_="2", created="t3"),
            ],
            me="me",
        )
        self.assertEqual(turn.turn, "gatekeeper")
        self.assertEqual(turn.currentVerdict, "clean")
        self.assertEqual(turn.verdictHead, "sha2")
        self.assertTrue(turn.atVerdictHead)
        self.assertIn("clean at head", turn.reason)

    def test_changes_requested_at_head_without_response_is_worker(self) -> None:
        turn = decide_pr_turn(
            head_sha="sha1",
            mergeable="MERGEABLE",
            comments=[gk("changes-requested", "sha1")],
            me="me",
        )
        self.assertEqual(turn.turn, "worker")

    def test_worker_response_at_same_head_is_gatekeeper(self) -> None:
        turn = decide_pr_turn(
            head_sha="sha1",
            mergeable="MERGEABLE",
            comments=[gk("changes-requested", "sha1"), wr("1", "sha1", "t2")],
            me="me",
        )
        self.assertEqual(turn.turn, "gatekeeper")

    def test_approve_at_head_is_human(self) -> None:
        turn = decide_pr_turn(
            head_sha="sha1",
            mergeable="MERGEABLE",
            comments=[gk("approve", "sha1")],
            me="me",
            labels=["gatekeeper:approved"],
        )
        self.assertEqual(turn.turn, "human")
        self.assertEqual(turn.labelMismatch, [])

    def test_stale_approved_label_at_changes_requested(self) -> None:
        mismatches = label_mismatches(
            ["gatekeeper:approved", "gatekeeper:changes-requested"],
            verdict="changes-requested",
            at_verdict_head=True,
        )
        self.assertIn("remove stale label gatekeeper:approved", mismatches)

    def test_merge_conflict_is_worker(self) -> None:
        turn = decide_pr_turn(
            head_sha="sha1",
            mergeable="CONFLICTING",
            comments=[],
            me="me",
        )
        self.assertEqual(turn.turn, "worker")


if __name__ == "__main__":
    unittest.main()
