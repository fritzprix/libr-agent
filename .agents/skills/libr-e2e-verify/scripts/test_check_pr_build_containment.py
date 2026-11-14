#!/usr/bin/env python3
"""Tests for check_pr_build_containment (gh and git are mocked)."""
import contextlib
import io
import json
import unittest
from pathlib import Path
from unittest.mock import patch

import check_pr_build_containment as cbc


class CheckPrBuildContainmentTest(unittest.TestCase):
    def test_is_ancestor_unknown_sha(self):
        repo = Path("/tmp/repo")
        self.assertFalse(cbc.is_ancestor(repo, "unknown", "abc"))
        self.assertFalse(cbc.is_ancestor(repo, "abc", "unknown"))

    def test_is_ancestor_delegates_to_git(self):
        repo = Path("/tmp/repo")

        def fake_run(cmd, **kwargs):
            self.assertEqual(cmd[:5], ["git", "-C", str(repo), "merge-base", "--is-ancestor"])
            return type("R", (), {"returncode": 0})()

        with patch.object(cbc.subprocess, "run", side_effect=fake_run):
            self.assertTrue(cbc.is_ancestor(repo, "aaa", "bbb"))

    def test_is_ancestor_exit_1_means_not_ancestor(self):
        repo = Path("/tmp/repo")
        fake = type("R", (), {"returncode": 1, "stderr": ""})()
        with patch.object(cbc.subprocess, "run", return_value=fake):
            self.assertFalse(cbc.is_ancestor(repo, "aaa", "bbb"))

    def test_is_ancestor_git_failure_raises_containment_error(self):
        # G1.2: exit 128 (git itself failed, e.g. sha not fetched) is an error,
        # not a "not in build" verdict.
        repo = Path("/tmp/repo")
        fake = type("R", (), {"returncode": 128, "stderr": "fatal: Not a valid object name bbb"})()
        with patch.object(cbc.subprocess, "run", return_value=fake):
            with self.assertRaises(cbc.ContainmentError) as ctx:
                cbc.is_ancestor(repo, "aaa", "bbb")
        self.assertIn("fatal: Not a valid object name bbb", str(ctx.exception))

    def test_provenance_from_url_reads_build_sha(self):
        payload = json.dumps({"buildSha": "deadbeef", "buildDirty": True}).encode("utf-8")

        class FakeResp:
            def read(self):
                return payload

            def __enter__(self):
                return self

            def __exit__(self, *_):
                return False

        with patch.object(cbc.urllib.request, "urlopen", return_value=FakeResp()):
            sha, dirty = cbc.provenance_from_url("http://127.0.0.1:3030/api/health", None)
        self.assertEqual(sha, "deadbeef")
        self.assertTrue(dirty)

    def test_main_json_with_build_sha(self):
        def fake_merge(pr):
            return f"merge{pr}"

        def fake_ancestor(repo, ancestor, descendant):
            return ancestor == "merge1"

        out = io.StringIO()
        with (
            patch.object(cbc, "merge_commit", side_effect=fake_merge),
            patch.object(cbc, "is_ancestor", side_effect=fake_ancestor),
            contextlib.redirect_stdout(out),
        ):
            code = cbc.main(["--prs", "1,2", "--build-sha", "headsha", "--json", "--repo", "."])
        self.assertEqual(code, 1)
        report = json.loads(out.getvalue())
        verdicts = {row["pr"]: row["verdict"] for row in report["prs"]}
        self.assertEqual(verdicts, {1: "contained", 2: "blocked: not in build"})
        # S1.2: --build-sha mode does not read buildDirty from the app.
        self.assertEqual(report["buildDirty"], "unknown")

    def test_main_git_error_exits_3(self):
        def fake_merge(pr):
            return f"merge{pr}"

        def boom(repo, ancestor, descendant):
            raise cbc.ContainmentError("git merge-base --is-ancestor failed (code 128)")

        err = io.StringIO()
        with (
            patch.object(cbc, "merge_commit", side_effect=fake_merge),
            patch.object(cbc, "is_ancestor", side_effect=boom),
            contextlib.redirect_stderr(err),
        ):
            code = cbc.main(["--prs", "1", "--build-sha", "headsha", "--repo", "."])
        self.assertEqual(code, 3)
        self.assertIn("code 128", err.getvalue())

    def test_main_bad_pr_number_exits_2(self):
        # S1.3: a non-numeric PR number is a usage error (2), not a traceback.
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            code = cbc.main(["--prs", "12a", "--build-sha", "headsha", "--repo", "."])
        self.assertEqual(code, 2)
        self.assertIn("--prs", err.getvalue())


if __name__ == "__main__":
    unittest.main()
