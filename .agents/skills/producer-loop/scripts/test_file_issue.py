#!/usr/bin/env python3
"""Tests for file_issue (gh is mocked)."""
import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import file_issue


def finding(key, body="Evidence: src/a.rs:10 handles input without a bound."):
    return {"key": key, "title": f"Problem {key}", "body": body}


class FakeGh:
    """Records gh calls; `issues` are existing issues returned by `issue list`."""

    def __init__(self, issues=None, labels=(), fail_on_create=None):
        self.issues = issues or []
        self.labels = list(labels)
        self.calls = []
        self.bodies = []
        self.next_number = 100
        self.fail_on_create = fail_on_create

    def __call__(self, *args, stdin=None):
        self.calls.append(args)
        if args[:2] == ("issue", "list"):
            return json.dumps(self.issues)
        if args[:2] == ("label", "list"):
            return json.dumps([{"name": n} for n in self.labels])
        if args[:2] == ("label", "create"):
            self.labels.append(args[2])
            return ""
        if args[:2] == ("issue", "create"):
            if self.fail_on_create == len(self.bodies):
                raise file_issue.GhError("gh issue create failed: rate limited")
            self.bodies.append(stdin)
            self.next_number += 1
            return f"https://github.com/o/r/issues/{self.next_number}\n"
        raise AssertionError(f"unexpected gh call: {args}")

    def created(self):
        return [c for c in self.calls if c[:2] == ("issue", "create")]


class FileIssueTest(unittest.TestCase):
    def run_findings(self, fake, task, findings, state=None, create=True):
        state = {} if state is None else state
        with patch.object(file_issue, "gh", fake):
            return file_issue.file_findings(task, findings, state, create), state

    def test_fingerprint_depends_on_task_and_key(self):
        a = file_issue.fingerprint("security", "k")
        self.assertEqual(a, file_issue.fingerprint("security", "k"))
        self.assertNotEqual(a, file_issue.fingerprint("e2e", "k"))
        self.assertNotEqual(a, file_issue.fingerprint("security", "k2"))

    def test_dry_run_creates_nothing(self):
        fake = FakeGh()
        results, state = self.run_findings(fake, "security", [finding("a")], create=False)
        self.assertEqual(results[0]["action"], "would-create")
        self.assertEqual(fake.created(), [])
        self.assertNotIn("a", json.dumps(state.get("fingerprints", {})))
        self.assertEqual(state.get("fingerprints", {}), {})

    def test_cli_defaults_to_dry_run(self):
        with tempfile.TemporaryDirectory() as tmp:
            findings = Path(tmp) / "f.json"
            findings.write_text(json.dumps([finding("a")]), encoding="utf-8")
            fake = FakeGh()
            out = io.StringIO()
            with patch.object(file_issue, "gh", fake), contextlib.redirect_stdout(out):
                code = file_issue.main(["--task", "security", "--findings", str(findings),
                                        "--state-file", str(Path(tmp) / "state.json")])
            self.assertEqual(code, 0)
            self.assertTrue(json.loads(out.getvalue())["dryRun"])
            self.assertEqual(fake.created(), [])
            self.assertFalse((Path(tmp) / "state.json").exists())

    def test_gh_failure_after_create_keeps_created_issue(self):
        with tempfile.TemporaryDirectory() as tmp:
            findings = Path(tmp) / "f.json"
            findings.write_text(json.dumps([finding("a"), finding("b")]), encoding="utf-8")
            state_file = Path(tmp) / "state.json"
            fake = FakeGh(fail_on_create=1)
            out = io.StringIO()
            with patch.object(file_issue, "gh", fake), contextlib.redirect_stdout(out), \
                    contextlib.redirect_stderr(io.StringIO()):
                code = file_issue.main(["--task", "security", "--findings", str(findings),
                                        "--state-file", str(state_file), "--create"])
            self.assertEqual(code, 3)
            fp = file_issue.fingerprint("security", "a")
            saved = json.loads(state_file.read_text(encoding="utf-8"))
            self.assertEqual(saved["fingerprints"][fp], 101)
            self.assertNotIn(file_issue.fingerprint("security", "b"), saved["fingerprints"])
            report = json.loads(out.getvalue())
            self.assertIn("rate limited", report["error"])
            self.assertEqual([r["url"] for r in report["results"] if r["action"] == "created"],
                             ["https://github.com/o/r/issues/101"])

    def test_create_adds_labels_marker_and_records_fingerprint(self):
        fake = FakeGh()
        results, state = self.run_findings(fake, "uiux", [finding("a")])
        self.assertEqual(results[0]["action"], "created")
        (call,) = fake.created()
        labels = [call[i + 1] for i, arg in enumerate(call) if arg == "--label"]
        self.assertEqual(labels, ["auto:producer", "auto:uiux"])
        fp = file_issue.fingerprint("uiux", "a")
        self.assertEqual(call[call.index("--body-file") + 1], "-")
        self.assertIn(file_issue.marker(fp), fake.bodies[0])
        self.assertEqual(state["fingerprints"][fp], results[0]["issue"])
        self.assertIn(("label", "create", "auto:uiux", "--description", "Created by producer-loop"), fake.calls)

    def test_existing_labels_are_not_recreated(self):
        fake = FakeGh(labels=["auto:producer", "auto:uiux"])
        self.run_findings(fake, "uiux", [finding("a")])
        self.assertFalse([c for c in fake.calls if c[:2] == ("label", "create")])

    def test_dedup_from_state(self):
        fake = FakeGh()
        fp = file_issue.fingerprint("security", "a")
        results, _ = self.run_findings(fake, "security", [finding("a")], state={"fingerprints": {fp: 7}})
        self.assertEqual(results[0]["action"], "skipped")
        self.assertIn("#7", results[0]["reason"])
        self.assertEqual(fake.calls, [])

    def test_dedup_from_open_or_closed_issue_marker(self):
        fp = file_issue.fingerprint("security", "a")
        fake = FakeGh(issues=[{"number": 42, "body": f"text\n{file_issue.marker(fp)}"}])
        results, state = self.run_findings(fake, "security", [finding("a")])
        self.assertEqual(results[0]["action"], "skipped")
        self.assertIn("#42", results[0]["reason"])
        self.assertEqual(state["fingerprints"][fp], 42)
        self.assertEqual(fake.created(), [])
        self.assertIn("--state", fake.calls[0])
        self.assertEqual(fake.calls[0][fake.calls[0].index("--state") + 1], "all")

    def test_search_hit_without_marker_is_not_a_duplicate(self):
        fp = file_issue.fingerprint("security", "a")
        fake = FakeGh(issues=[{"number": 5, "body": f"mentions {fp} only"}])
        results, _ = self.run_findings(fake, "security", [finding("a")])
        self.assertEqual(results[0]["action"], "created")

    def test_cap_limits_new_issues_per_run(self):
        fake = FakeGh()
        findings = [finding(str(i)) for i in range(file_issue.MAX_NEW_ISSUES_PER_RUN + 2)]
        results, _ = self.run_findings(fake, "e2e", findings)
        self.assertEqual(len(fake.created()), file_issue.MAX_NEW_ISSUES_PER_RUN)
        skipped = [r for r in results if r["action"] == "skipped"]
        self.assertEqual(len(skipped), 2)
        self.assertIn("cap", skipped[0]["reason"])

    def test_cap_override_per_task(self):
        fake = FakeGh()
        state = {"maxNewIssues": {"security": 1}}
        self.run_findings(fake, "security", [finding("a"), finding("b")], state=state)
        self.assertEqual(len(fake.created()), 1)
        fake2 = FakeGh()
        self.run_findings(fake2, "e2e", [finding("a"), finding("b")], state=state)
        self.assertEqual(len(fake2.created()), 2)

    def test_dedup_skips_do_not_use_the_cap(self):
        fake = FakeGh()
        known = {file_issue.fingerprint("e2e", str(i)): i for i in range(5)}
        findings = [finding(str(i)) for i in range(5)] + [finding("new")]
        results, _ = self.run_findings(fake, "e2e", findings, state={"fingerprints": known})
        self.assertEqual(results[-1]["action"], "created")

    def test_private_text_is_refused(self):
        for body in ("token = ghp_abcdefghijklmnopqrstuvwxyz0123",
                     "see /home/alice/project/file.rs",
                     "-----BEGIN RSA PRIVATE KEY-----"):
            fake = FakeGh()
            results, _ = self.run_findings(fake, "security", [finding("a", body)])
            self.assertEqual(results[0]["action"], "skipped", body)
            self.assertEqual(fake.created(), [])

    def test_invalid_finding_is_skipped(self):
        results, _ = self.run_findings(FakeGh(), "security", [{"key": "a"}])
        self.assertEqual(results[0]["action"], "skipped")

    def test_only_create_commands_are_used(self):
        fake = FakeGh()
        self.run_findings(fake, "security", [finding("a")])
        used = {c[:2] for c in fake.calls}
        self.assertTrue(used <= {("issue", "list"), ("issue", "create"), ("label", "list"), ("label", "create")})


if __name__ == "__main__":
    unittest.main()
