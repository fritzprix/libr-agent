#!/usr/bin/env python3
"""Tests for checklist (gh is mocked)."""
import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import checklist

TAGGED = """## Checklist
### 1. Area
- **E-1** [auto] – new session → run setup with `dummy-token-abcd` → exit 3 (#2562, source: PR body)
- **E-2** [manual] – dialog → enter value → masked (#2562, source: issue AC)
- **E-3** [blocked: #2557] – restart → record text (#2562, source: issue AC)
- **E-4** [auto] – run `echo $DEMO_TOKEN` → prints *** (#2554, source: issue AC)
"""

# Old human format (#2565): no [tag], some IDs carry notes inside the bold text.
UNTAGGED_2565 = """## Checklist
Format: **ID** – precondition → steps → expected result (PR).
- **P-1** – new session; Linux → ask the agent to run `sudo -k; sudo true` → one dialog (#2551).
- **P-8 (blocked: #2557)** – a prompt waits → restart the app → record this only.
- **B-1 (Linux)** – sudo account → run sudo → passes (#2552).
- **B-3** – store `dummy-token-abcd` → masked (#2554).
"""


class FakeGh:
    def __init__(self, body, comments=()):
        self.body = body
        self.comments = list(comments)
        self.calls = []

    def __call__(self, *args, stdin=None):
        self.calls.append((args, stdin))
        if args[:2] == ("issue", "view"):
            return json.dumps({"body": self.body, "comments": self.comments})
        if args[:2] in (("issue", "comment"), ("api", "-X")):
            return ""
        raise AssertionError(f"unexpected gh call: {args}")


class ChecklistTest(unittest.TestCase):
    def test_only_auto_items_are_picked(self):
        auto, untouched = checklist.split_items(checklist.parse_items(TAGGED))
        self.assertEqual([i["id"] for i in auto], ["E-1", "E-4"])
        self.assertEqual([(i["id"], i["tag"]) for i in untouched], [("E-2", "manual"), ("E-3", "blocked: #2557")])

    def test_untagged_2565_format_has_no_auto_items(self):
        items = checklist.parse_items(UNTAGGED_2565)
        self.assertEqual([i["id"] for i in items], ["P-1", "P-8", "B-1", "B-3"])
        auto, untouched = checklist.split_items(items)
        self.assertEqual(auto, [])
        self.assertEqual(len(untouched), 4)
        comment = checklist.render_comment(2565, items, [])
        self.assertIn(checklist.UNTAGGED_NOTE, comment)
        self.assertIn("Untouched (4): untagged: P-1, P-8, B-1, B-3.", comment)
        self.assertTrue(comment.startswith(checklist.results_marker(2565)))

    def test_comment_action_updates_marked_comment_or_creates(self):
        marked = {"url": "https://github.com/o/r/issues/2565#issuecomment-12345",
                  "body": f"{checklist.results_marker(2565)}\nold"}
        other = {"url": "https://github.com/o/r/issues/2565#issuecomment-1", "body": "hello"}
        self.assertEqual(checklist.comment_action([other, marked], 2565), ("update", "12345"))
        self.assertEqual(checklist.comment_action([other], 2565), ("create", None))
        wrong_issue = {"url": marked["url"], "body": checklist.results_marker(9)}
        self.assertEqual(checklist.comment_action([wrong_issue], 2565), ("create", None))

    def test_dummy_secret_in_result_fails_and_is_scrubbed(self):
        secrets = checklist.dummy_secrets(TAGGED)
        self.assertEqual(secrets, ["dummy-token-abcd"])
        rows = checklist.apply_results([
            {"id": "E-1", "verdict": "pass", "session": "s1", "excerpt": "exit 3, hint shown"},
            {"id": "E-4", "verdict": "pass", "session": "s2", "excerpt": "output: dummy-token-abcd",
             "artifact": "work/dummy-token-abcd.log"},
        ], secrets)
        self.assertEqual(rows[0]["verdict"], "pass")
        self.assertEqual(rows[1]["verdict"], "fail")
        rendered = checklist.render_comment(1, checklist.parse_items(TAGGED), rows)
        self.assertNotIn("dummy-token-abcd", rendered)
        self.assertIn("***", rendered)
        self.assertIn("| E-4 | fail |", rendered)

    def test_bad_verdict_is_refused(self):
        with self.assertRaises(ValueError):
            checklist.apply_results([{"id": "E-1", "verdict": "ok"}], [])

    def run_main(self, fake, *argv):
        out = io.StringIO()
        with patch.object(checklist, "gh", fake), contextlib.redirect_stdout(out):
            code = checklist.main(list(argv))
        return code, out.getvalue()

    def test_comment_defaults_to_dry_run(self):
        fake = FakeGh(UNTAGGED_2565)
        code, out = self.run_main(fake, "comment", "--issue", "2565")
        self.assertEqual(code, 0)
        self.assertIn("ACTION create (dry run)", out)
        self.assertEqual([c for c, _ in fake.calls if c[:2] != ("issue", "view")], [])

    def test_post_updates_existing_comment(self):
        marked = {"url": "https://github.com/o/r/issues/1#issuecomment-77", "body": checklist.results_marker(1)}
        fake = FakeGh(TAGGED, [marked])
        with tempfile.TemporaryDirectory() as tmp:
            results = Path(tmp) / "r.json"
            results.write_text(json.dumps([{"id": "E-1", "verdict": "pass", "session": "s1"}]), encoding="utf-8")
            code, out = self.run_main(fake, "comment", "--issue", "1", "--results", str(results), "--post")
        self.assertEqual(code, 0)
        self.assertIn("ACTION update 77", out)
        args, stdin = fake.calls[-1]
        self.assertIn("repos/{owner}/{repo}/issues/comments/77", args)
        self.assertIn("| E-1 | pass |", json.loads(stdin)["body"])

    def test_post_creates_comment_when_none_exists(self):
        fake = FakeGh(TAGGED)
        code, _ = self.run_main(fake, "comment", "--issue", "1", "--post")
        self.assertEqual(code, 0)
        args, stdin = fake.calls[-1]
        self.assertEqual(args[:3], ("issue", "comment", "1"))
        self.assertIn("2 `[auto]` items found. No results yet.", stdin)


if __name__ == "__main__":
    unittest.main()
