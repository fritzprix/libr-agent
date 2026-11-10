#!/usr/bin/env python3
"""Tests for issue_validation (gh is mocked)."""
import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import issue_validation


def issue(number, labels, created="2026-10-30T00:00:00Z", state="OPEN", body="Finding.\n\nEvidence: a.py:1"):
    return {"number": number, "title": f"Issue {number}", "state": state, "body": body,
            "createdAt": created, "labels": [{"name": name} for name in labels]}


class FakeGh:
    def __init__(self, issues, labels=("validated", "rejected")):
        self.issues = {item["number"]: item for item in issues}
        self.labels = list(labels)
        self.calls = []
        self.edited_bodies = {}

    def __call__(self, *args, stdin=None):
        self.calls.append(args)
        if args[:2] == ("issue", "list"):
            return json.dumps(list(self.issues.values()))
        if args[:2] == ("issue", "view"):
            return json.dumps(self.issues[int(args[2])])
        if args[:2] == ("issue", "edit") and "--body-file" in args:
            self.edited_bodies[int(args[2])] = stdin
            return ""
        if args[:2] == ("issue", "edit") and "--add-label" in args:
            return ""
        if args[:2] == ("label", "list"):
            return json.dumps([{"name": n} for n in self.labels])
        if args[:2] == ("label", "create"):
            self.labels.append(args[2])
            return ""
        raise AssertionError(f"unexpected gh call: {args}")

    def added_labels(self):
        return [c[c.index("--add-label") + 1] for c in self.calls if "--add-label" in c]


class NextIssueTest(unittest.TestCase):
    def test_picks_oldest_unjudged_producer_issue(self):
        fake = FakeGh([
            issue(3, ["auto:producer", "auto:security"], created="2026-10-29T00:00:00Z"),
            issue(1, ["auto:producer", "auto:e2e", "validated"], created="2026-10-01T00:00:00Z"),
            issue(2, ["auto:producer", "auto:uiux", "rejected"], created="2026-10-02T00:00:00Z"),
            issue(4, ["auto:producer", "auto:cleanup"], created="2026-10-30T00:00:00Z"),
        ])
        with patch.object(issue_validation, "gh", fake):
            picked = issue_validation.next_issue()
        self.assertEqual(picked["number"], 3)
        self.assertEqual(picked["task"], "security")

    def test_judged_issues_are_skipped(self):
        fake = FakeGh([issue(1, ["auto:producer", "auto:e2e", "validated"]),
                       issue(2, ["auto:producer", "auto:e2e", "rejected"])])
        out = io.StringIO()
        with patch.object(issue_validation, "gh", fake), contextlib.redirect_stdout(out):
            code = issue_validation.main(["next"])
        self.assertEqual(code, 1)
        self.assertEqual(out.getvalue().strip(), "NONE")


class JudgeTest(unittest.TestCase):
    def run_judge(self, fake, number, verdict, state=None, reason="Confirmed at src/a.rs:10."):
        state = {} if state is None else state
        with patch.object(issue_validation, "gh", fake):
            return issue_validation.judge(number, verdict, reason, state), state

    def test_validated_appends_evidence_and_counts_per_task(self):
        fake = FakeGh([issue(7, ["auto:producer", "auto:security"])])
        result, state = self.run_judge(fake, 7, "validated")
        self.assertEqual(fake.added_labels(), ["validated"])
        body = fake.edited_bodies[7]
        self.assertTrue(body.startswith("Finding.\n\nEvidence: a.py:1"))
        self.assertIn("verdict=validated", body)
        self.assertIn("Confirmed at src/a.rs:10.", body)
        self.assertEqual(state["issueValidation"]["security"], {"validated": 1, "rejected": 0})
        self.assertEqual(result["verdict"], "validated")

    def test_rejected_counts_separately(self):
        state = {"issueValidation": {"security": {"validated": 2, "rejected": 0}}}
        fake = FakeGh([issue(8, ["auto:producer", "auto:security"])])
        self.run_judge(fake, 8, "rejected", state=state, reason="Already fixed in src/a.rs:12.")
        self.assertEqual(fake.added_labels(), ["rejected"])
        self.assertEqual(state["issueValidation"]["security"], {"validated": 2, "rejected": 1})

    def test_already_judged_or_foreign_issue_is_refused(self):
        for labels in (["auto:producer", "auto:e2e", "validated"], ["bug"]):
            fake = FakeGh([issue(9, labels)])
            with self.assertRaises(ValueError, msg=labels):
                self.run_judge(fake, 9, "validated")
            self.assertEqual(fake.edited_bodies, {})
            self.assertEqual(fake.added_labels(), [])

    def test_closed_issue_is_refused(self):
        fake = FakeGh([issue(10, ["auto:producer", "auto:e2e"], state="CLOSED")])
        with self.assertRaises(ValueError):
            self.run_judge(fake, 10, "validated")

    def test_empty_reason_is_refused(self):
        fake = FakeGh([issue(11, ["auto:producer", "auto:e2e"])])
        with self.assertRaises(ValueError):
            self.run_judge(fake, 11, "validated", reason="  ")
        self.assertEqual(fake.calls, [])

    def test_never_approves_closes_or_comments(self):
        fake = FakeGh([issue(12, ["auto:producer", "auto:e2e"])], labels=())
        self.run_judge(fake, 12, "validated")
        self.assertNotIn("approved", fake.added_labels())
        used = {c[:2] for c in fake.calls}
        self.assertTrue(used <= {("issue", "view"), ("issue", "edit"), ("label", "list"), ("label", "create")})
        self.assertIn(("label", "create", "validated", "--description",
                       "Set by gatekeeper-loop issue validation"), fake.calls)

    def test_cli_saves_counts_to_state(self):
        with tempfile.TemporaryDirectory() as tmp:
            reason = Path(tmp) / "reason.md"
            reason.write_text("Confirmed at src/a.rs:10.", encoding="utf-8")
            state_file = Path(tmp) / "state.json"
            state_file.write_text(json.dumps({"prs": {"1": {}}}), encoding="utf-8")
            fake = FakeGh([issue(13, ["auto:producer", "auto:doc-drift"])])
            with patch.object(issue_validation, "gh", fake), contextlib.redirect_stdout(io.StringIO()):
                code = issue_validation.main(["--state-file", str(state_file), "judge", "13",
                                              "--verdict", "rejected", "--reason-file", str(reason)])
            self.assertEqual(code, 0)
            saved = json.loads(state_file.read_text(encoding="utf-8"))
            self.assertEqual(saved["prs"], {"1": {}})
            self.assertEqual(saved["issueValidation"]["doc-drift"], {"validated": 0, "rejected": 1})


if __name__ == "__main__":
    unittest.main()
