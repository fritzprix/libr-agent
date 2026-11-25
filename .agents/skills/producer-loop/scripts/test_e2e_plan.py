#!/usr/bin/env python3
"""Tests for e2e_plan (gh is mocked)."""
import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import e2e_plan
import file_issue

CURSOR = e2e_plan.DEFAULT_CURSOR


def pr(number, merged_at, paths, body="Fixes #900"):
    return {"number": number, "title": f"PR {number}", "mergedAt": merged_at, "body": body,
            "files": [{"path": p} for p in paths]}


HUMAN_ISSUE_2565 = """## Summary
Verify the PRs.

## Scope
- Merged: #2509, #2561.
- Pending merge: #2562, #2563 (#2546 parts 4 and 5).
- Out of scope (no product change): #2511 (bench script).

## Checklist
- **P-1** – something (#2562).
"""


class FakeGh:
    def __init__(self, prs=(), issues=()):
        self.prs = list(prs)
        self.issues = list(issues)
        self.calls = []

    def __call__(self, *args):
        self.calls.append(args)
        if args[:2] == ("pr", "list"):
            return json.dumps(self.prs)
        if args[:2] == ("issue", "list"):
            return json.dumps(self.issues)
        if args[:2] == ("issue", "view"):
            return json.dumps({"number": int(args[2]), "title": "Linked",
                               "body": "## Context\nx\n## Requirements\n1. Do it.\n## Acceptance criteria\n- It works."})
        raise AssertionError(f"unexpected gh call: {args}")


def items():
    return {
        "top10": ["E-1: the password is not in the output (#2562)."],
        "areas": [{"prefix": "E", "title": "email-integration setup", "items": [
            {"tag": "auto", "text": "new session → run setup with `dummy-token-abcd` → exit 0", "prs": [2562],
             "source": "issue AC"},
            {"tag": "manual", "text": "dialog → enter value → masked", "prs": [2562], "source": "PR body"},
            {"tag": "blocked: #2557", "text": "restart → crash text", "prs": [2562], "source": "PR body"},
        ]}],
    }


class E2ePlanTest(unittest.TestCase):
    def collect(self, fake, state=None):
        with patch.object(e2e_plan, "gh", fake):
            return e2e_plan.collect({} if state is None else state)

    def test_only_prs_after_the_cursor_are_picked(self):
        fake = FakeGh(prs=[pr(2561, CURSOR, ["src/a.ts"]),
                           pr(2563, "2026-11-04T09:00:00Z", ["src-tauri/src/b.rs"]),
                           pr(2562, "2026-11-04T08:00:00Z", ["src/c.ts"])])
        result = self.collect(fake)
        self.assertEqual(result["action"], "plan")
        self.assertEqual([p["number"] for p in result["prs"]], [2562, 2563])
        self.assertEqual(result["range"], [2562, 2563])
        self.assertEqual(result["cursorAfter"], "2026-11-04T09:00:00Z")
        search = next(c for c in fake.calls if c[:2] == ("pr", "list"))
        self.assertIn(f"merged:>{CURSOR}", search)

    def test_cursor_from_state_wins(self):
        fake = FakeGh(prs=[pr(2562, "2026-11-04T08:00:00Z", ["src/c.ts"])])
        result = self.collect(fake, {e2e_plan.CURSOR_KEY: "2026-11-04T08:30:00Z"})
        self.assertEqual(result["action"], "noop")

    def test_non_product_prs_are_excluded(self):
        fake = FakeGh(prs=[
            pr(1, "2026-11-04T08:00:00Z", [".agents/skills/x/SKILL.md", "benchmarks/a.py"]),
            pr(2, "2026-11-04T08:01:00Z", ["scripts/run-harbor-bench.sh", "docs/a.md"]),
            pr(3, "2026-11-04T08:02:00Z", ["src-tauri/tests/a.rs", "src/a.test.ts", "x/scripts/test_setup.py"]),
            pr(4, "2026-11-04T08:03:00Z", ["src-tauri/bundled_skills/x/scripts/setup.py", "docs/a.md"]),
        ])
        result = self.collect(fake)
        self.assertEqual([p["number"] for p in result["prs"]], [4])
        self.assertEqual([e["number"] for e in result["excluded"]], [1, 2, 3])

    def test_prs_in_open_e2e_scope_are_excluded_even_without_marker(self):
        fake = FakeGh(prs=[pr(2561, "2026-11-04T08:00:00Z", ["src/a.ts"]),
                           pr(2562, "2026-11-04T08:01:00Z", ["src/b.ts"])],
                      issues=[{"number": 2565, "body": HUMAN_ISSUE_2565}])
        result = self.collect(fake)
        self.assertEqual([p["number"] for p in result["prs"]], [2562])
        self.assertIn("#2565", result["excluded"][0]["reason"])
        listing = next(c for c in fake.calls if c[:2] == ("issue", "list"))
        self.assertEqual(listing[listing.index("--label") + 1], "auto:e2e")

    def test_scope_parse_skips_pending_lines(self):
        self.assertEqual(e2e_plan.scope_prs(HUMAN_ISSUE_2565), {2509, 2561, 2511})

    def test_zero_product_prs_is_a_noop(self):
        fake = FakeGh(prs=[pr(1, "2026-11-04T08:00:00Z", [".agents/skills/a.md"])])
        result = self.collect(fake)
        self.assertEqual(result["action"], "noop")
        self.assertEqual(result["reason"], "no product PRs")
        self.assertNotIn("cursorAfter", result)

    def test_zero_product_prs_leaves_the_cursor(self):
        with tempfile.TemporaryDirectory() as tmp:
            state_file = Path(tmp) / "state.json"
            fake = FakeGh(prs=[pr(1, "2026-11-04T08:00:00Z", ["docs/a.md"])])
            out = io.StringIO()
            with patch.object(e2e_plan, "gh", fake), contextlib.redirect_stdout(out):
                code = e2e_plan.main(["--state-file", str(state_file), "collect"])
            self.assertEqual(code, 0)
            self.assertEqual(json.loads(out.getvalue())["action"], "noop")
            self.assertFalse(state_file.exists())

    def test_same_range_marker_is_skipped(self):
        fake = FakeGh(prs=[pr(2562, "2026-11-04T08:00:00Z", ["src/a.ts"])],
                      issues=[{"number": 7, "body": f"## Summary\nx\n{e2e_plan.range_marker(2562, 2562)}"}])
        result = self.collect(fake)
        self.assertEqual(result["action"], "noop")
        self.assertIn("#7", result["reason"])

    def test_linked_issue_requirements_are_collected(self):
        fake = FakeGh(prs=[pr(2562, "2026-11-04T08:00:00Z", ["src/a.ts"], body="Part 4 of #2546. Refs #2544")])
        result = self.collect(fake)
        linked = result["prs"][0]["linkedIssues"]
        self.assertEqual([i["number"] for i in linked], [2544, 2546])
        self.assertIn("## Requirements", linked[0]["requirements"])
        self.assertNotIn("## Context", linked[0]["requirements"])

    def collected(self):
        fake = FakeGh(prs=[pr(2562, "2026-11-04T08:00:00Z", ["src/a.ts"]),
                           pr(9, "2026-11-04T08:01:00Z", ["docs/a.md"])])
        return self.collect(fake)

    def test_rendered_body_has_tags_sources_and_acceptance_criteria(self):
        body = e2e_plan.render_body(self.collected(), items())
        self.assertIn("- **E-1** [auto] – new session", body)
        self.assertIn("(#2562, source: issue AC)", body)
        self.assertIn("- **E-2** [manual]", body)
        self.assertIn("- **E-3** [blocked: #2557]", body)
        self.assertIn("## Acceptance criteria", body)
        self.assertIn("- Out of scope: #9 (no product change", body)
        self.assertIn("e2e-build-precondition.md", body)
        self.assertIn(e2e_plan.range_marker(2562, 2562), body)
        self.assertIsNone(file_issue.private_text_reason(body))
        findings = e2e_plan.render_findings(self.collected(), items())
        self.assertEqual(findings[0]["key"], "e2e-plan:2562-2562")
        self.assertTrue(findings[0]["title"].startswith("E2E verification: PRs merged into dev/0.9.x"))

    def test_render_refuses_bad_tags_sources_and_secrets(self):
        for mutate in (lambda d: d["areas"][0]["items"][0].update(tag="maybe"),
                       lambda d: d["areas"][0]["items"][0].update(source="guess"),
                       lambda d: d["areas"][0]["items"][0].update(prs=[]),
                       lambda d: d["areas"][0]["items"][0].update(text="password = hunter2hunter2")):
            data = items()
            mutate(data)
            with self.assertRaises(ValueError):
                e2e_plan.render_body(self.collected(), data)

    def test_advance_moves_cursor_forward_only(self):
        state = {}
        self.assertFalse(e2e_plan.advance(state, CURSOR))
        self.assertTrue(e2e_plan.advance(state, "2026-11-04T09:00:00Z"))
        self.assertFalse(e2e_plan.advance(state, "2026-11-04T08:00:00Z"))
        self.assertEqual(state[e2e_plan.CURSOR_KEY], "2026-11-04T09:00:00Z")


if __name__ == "__main__":
    unittest.main()
