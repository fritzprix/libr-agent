#!/usr/bin/env python3
"""Tests for check_pr_build_containment (gh and git are mocked)."""
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

        def fake_run(cmd, capture_output):
            self.assertEqual(cmd[:5], ["git", "-C", str(repo), "merge-base", "--is-ancestor"])
            return type("R", (), {"returncode": 0})()

        with patch.object(cbc.subprocess, "run", side_effect=fake_run):
            self.assertTrue(cbc.is_ancestor(repo, "aaa", "bbb"))

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

        with (
            patch.object(cbc, "merge_commit", side_effect=fake_merge),
            patch.object(cbc, "is_ancestor", side_effect=fake_ancestor),
        ):
            code = cbc.main(["--prs", "1,2", "--build-sha", "headsha", "--json", "--repo", "."])
        self.assertEqual(code, 1)


if __name__ == "__main__":
    unittest.main()
