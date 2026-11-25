#!/usr/bin/env python3
"""Tests for the fixed loop marker kinds (issue #2994).

Run: python3 .agents/skills/loop-shared/scripts/test_comment_markers.py
"""
from __future__ import annotations

import unittest
from pathlib import Path
import sys

SCRIPTS = Path(__file__).resolve().parent
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from comment_markers import (  # noqa: E402
    GATEKEEPER_VERDICTS,
    WORKER_KINDS,
    parse_markers,
    reply_targets,
)


def comment(body: str, login: str = "me", created: str = "t1") -> dict:
    return {"author": {"login": login}, "createdAt": created, "body": body, "url": "u"}


class FixedKindListTests(unittest.TestCase):
    def test_gatekeeper_verdicts_are_the_fixed_set(self):
        self.assertEqual(
            set(GATEKEEPER_VERDICTS),
            {"changes-requested", "approve", "clean", "escalate"},
        )

    def test_worker_kinds_are_the_fixed_set(self):
        self.assertEqual(
            set(WORKER_KINDS),
            {"response", "needs-owner", "ci-attempt", "reply"},
        )


class ParseMarkersFixedKindsTests(unittest.TestCase):
    def test_unknown_gatekeeper_verdict_is_ignored(self):
        comments = [comment("<!-- gatekeeper-loop: verdict=ci-report head=sha1 -->")]
        verdicts, responses = parse_markers(comments, "me")
        self.assertEqual(verdicts, [])
        self.assertEqual(responses, [])

    def test_unknown_worker_kind_is_ignored(self):
        # Kinds used on PR #2990 before the fixed list existed.
        comments = [
            comment("<!-- worker-loop: ci-report round=1 head=sha1 -->"),
            comment("<!-- worker-loop: ci-rerun round=1 head=sha1 -->"),
        ]
        verdicts, responses = parse_markers(comments, "me")
        self.assertEqual(verdicts, [])
        self.assertEqual(responses, [])

    def test_only_response_kind_counts_as_a_response(self):
        comments = [
            comment("<!-- worker-loop: response round=1 head=sha2 -->", created="t2"),
            comment("<!-- worker-loop: needs-owner -->", created="t3"),
            comment("<!-- worker-loop: ci-attempt n=1 -->", created="t4"),
            comment("<!-- worker-loop: reply reply-to=42 -->", created="t5"),
        ]
        _, responses = parse_markers(comments, "me")
        self.assertEqual([r["createdAt"] for r in responses], ["t2"])
        self.assertEqual(responses[0]["kind"], "response")

    def test_clean_marker_accepts_no_blocked_by_field(self):
        comments = [comment("<!-- gatekeeper-loop: verdict=clean head=sha1 blocked-by=2995 -->")]
        verdicts, _ = parse_markers(comments, "me")
        self.assertEqual(len(verdicts), 1)
        self.assertNotIn("blocked-by", verdicts[0])
        self.assertEqual(verdicts[0]["verdict"], "clean")

    def test_fixed_gatekeeper_verdicts_are_parsed(self):
        comments = [
            comment(f"<!-- gatekeeper-loop: verdict={v} round=1 head=sha1 -->", created=f"c{i}")
            for i, v in enumerate(sorted(GATEKEEPER_VERDICTS))
        ]
        verdicts, _ = parse_markers(comments, "me")
        self.assertEqual(len(verdicts), len(GATEKEEPER_VERDICTS))


class ReplyTargetsTests(unittest.TestCase):
    def test_reply_marker_collects_reply_to_id(self):
        comments = [
            comment("<!-- worker-loop: reply reply-to=101 -->"),
            comment("<!-- worker-loop: reply reply-to=202 -->", created="t2"),
        ]
        self.assertEqual(reply_targets(comments, "me"), {"101", "202"})

    def test_reply_to_from_other_authors_and_other_kinds_is_ignored(self):
        comments = [
            comment("<!-- worker-loop: reply reply-to=101 -->", login="someone-else"),
            comment("<!-- worker-loop: response reply-to=303 round=1 -->"),
            comment("<!-- gatekeeper-loop: verdict=clean reply-to=404 -->"),
        ]
        self.assertEqual(reply_targets(comments, "me"), set())


if __name__ == "__main__":
    unittest.main()
