#!/usr/bin/env python3
"""Tests for shared PR turn ownership."""
from __future__ import annotations

import unittest
from pathlib import Path
import sys

SCRIPTS = Path(__file__).resolve().parent
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from pr_turn import decide_pr_turn, label_mismatches, overlap_rank  # noqa: E402


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

    def test_escalate_without_the_label_is_the_gatekeeper_turn(self) -> None:
        """G1.1: the owner removed the label, so no label fix may re-add it."""
        turn = decide_pr_turn(
            head_sha="sha2",
            mergeable="MERGEABLE",
            comments=[gk("escalate", "sha2")],
            me="me",
            labels=["gatekeeper:needs-human"],
        )
        self.assertEqual(turn.turn, "gatekeeper")
        self.assertEqual(turn.labelMismatch, [])
        self.assertIn("resumed", turn.reason)

    def test_escalate_with_the_label_stays_with_the_human(self) -> None:
        turn = decide_pr_turn(
            head_sha="sha2",
            mergeable="MERGEABLE",
            comments=[gk("escalate", "sha2")],
            me="me",
            labels=["gatekeeper:escalated"],
        )
        self.assertEqual(turn.turn, "human")
        self.assertEqual(turn.labelMismatch, [])

    def test_escalate_with_unknown_labels_stays_with_the_human(self) -> None:
        turn = decide_pr_turn(
            head_sha="sha2",
            mergeable="MERGEABLE",
            comments=[gk("escalate", "sha2")],
            me="me",
        )
        self.assertEqual(turn.turn, "human")

    def test_merge_conflict_is_worker(self) -> None:
        turn = decide_pr_turn(
            head_sha="sha1",
            mergeable="CONFLICTING",
            comments=[],
            me="me",
        )
        self.assertEqual(turn.turn, "worker")


class FixedMarkerKindsTests(unittest.TestCase):
    """Issue #2994: unknown markers are ignored; no turn is ever a stall state."""

    def test_unknown_verdict_marker_does_not_change_the_turn(self) -> None:
        """A marker with a verdict outside the fixed list is not a verdict at all."""
        turn = decide_pr_turn(
            head_sha="sha1",
            mergeable="MERGEABLE",
            comments=[{"author": {"login": "me"}, "createdAt": "t1",
                       "body": "<!-- gatekeeper-loop: verdict=ci-report head=sha1 -->"}],
            me="me",
        )
        self.assertEqual(turn.turn, "gatekeeper")
        self.assertEqual(turn.currentVerdict, None)
        self.assertEqual(turn.reason, "not reviewed yet")

    def test_unknown_verdict_does_not_supersede_the_real_verdict(self) -> None:
        turn = decide_pr_turn(
            head_sha="sha1",
            mergeable="MERGEABLE",
            comments=[
                gk("changes-requested", "sha1"),
                {"author": {"login": "me"}, "createdAt": "t2",
                 "body": "<!-- gatekeeper-loop: verdict=ci-report head=sha1 -->"},
            ],
            me="me",
        )
        self.assertEqual(turn.turn, "worker")
        self.assertEqual(turn.currentVerdict, "changes-requested")

    def test_unknown_worker_kind_does_not_answer_a_round(self) -> None:
        turn = decide_pr_turn(
            head_sha="sha1",
            mergeable="MERGEABLE",
            comments=[
                gk("changes-requested", "sha1"),
                {"author": {"login": "me"}, "createdAt": "t2",
                 "body": "<!-- worker-loop: ci-report round=1 head=sha1 -->"},
            ],
            me="me",
        )
        self.assertEqual(turn.turn, "worker")

    def test_reply_marker_does_not_answer_a_round(self) -> None:
        """`reply` answers a triage comment, not the gatekeeper round."""
        turn = decide_pr_turn(
            head_sha="sha1",
            mergeable="MERGEABLE",
            comments=[
                gk("changes-requested", "sha1"),
                {"author": {"login": "me"}, "createdAt": "t2",
                 "body": "<!-- worker-loop: reply reply-to=42 -->"},
            ],
            me="me",
        )
        self.assertEqual(turn.turn, "worker")

    def test_every_fixed_verdict_combination_has_the_exact_owner(self) -> None:
        """G1.2: assert the expected owner for each verdict, head, and response."""
        expected = {
            # (verdict, verdict head, worker response?): turn
            ("changes-requested", "sha1", None): "gatekeeper",
            ("changes-requested", "sha1", "response"): "gatekeeper",
            ("changes-requested", "sha2", None): "worker",
            ("changes-requested", "sha2", "response"): "gatekeeper",
            ("approve", "sha1", None): "gatekeeper",
            ("approve", "sha1", "response"): "gatekeeper",
            ("approve", "sha2", None): "human",
            ("approve", "sha2", "response"): "human",
            ("clean", "sha1", None): "gatekeeper",
            ("clean", "sha1", "response"): "gatekeeper",
            ("clean", "sha2", None): "gatekeeper",
            ("clean", "sha2", "response"): "gatekeeper",
            ("escalate", "sha1", None): "gatekeeper",
            ("escalate", "sha1", "response"): "gatekeeper",
            ("escalate", "sha2", None): "human",
            ("escalate", "sha2", "response"): "human",
        }
        for (verdict, head, response), expected_turn in expected.items():
            comments = [gk(verdict, head)]
            if response:
                comments.append(wr("1", head, "t2"))
            turn = decide_pr_turn(
                head_sha="sha2", mergeable="MERGEABLE", comments=comments, me="me"
            )
            self.assertEqual(
                turn.turn, expected_turn,
                f"verdict={verdict} verdictHead={head} response={response}",
            )


class OverlapRankTest(unittest.TestCase):
    """Issue #2995: shared order for overlapping PR/issue states."""

    def test_fixed_order(self):
        # needs-owner > escalated > open blocker > merge conflict > verdict label
        self.assertEqual(
            overlap_rank(["merge-conflict", "needs-owner", "open-blocker"]), 0
        )
        self.assertEqual(overlap_rank(["gatekeeper:escalated", "open-blocker"]), 1)
        self.assertEqual(overlap_rank(["open-blocker", "merge-conflict"]), 2)
        self.assertEqual(overlap_rank(["verdict-label", "merge-conflict"]), 3)
        self.assertEqual(overlap_rank(["verdict-label"]), 4)

    def test_unknown_names_are_ignored(self):
        self.assertEqual(overlap_rank(["whatever"]), None)
        self.assertEqual(overlap_rank([]), None)
        self.assertIsNone(overlap_rank(None))


if __name__ == "__main__":
    unittest.main()
