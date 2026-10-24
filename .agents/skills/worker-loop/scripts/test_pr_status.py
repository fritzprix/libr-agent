#!/usr/bin/env python3
"""Tests for pr_status.duplicate_commits. Run: python3 -m unittest discover -s .agents/skills/worker-loop/scripts"""
import subprocess
import tempfile
import unittest
from pathlib import Path

import pr_status


def git(cwd: str, *args: str) -> str:
    return subprocess.run(
        ["git", "-c", "user.name=t", "-c", "user.email=t@t", *args],
        cwd=cwd, check=True, capture_output=True, text=True,
    ).stdout.strip()


def commit(cwd: str, name: str) -> str:
    Path(cwd, name).write_text(name)
    git(cwd, "add", name)
    git(cwd, "commit", "-m", name)
    return git(cwd, "rev-parse", "HEAD")


class DuplicateCommitsTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.repo = self._tmp.name
        git(self.repo, "init", "-q", "-b", "main")
        commit(self.repo, "root")

    def push_ref(self, branch: str):
        git(self.repo, "update-ref", f"refs/remotes/origin/{branch}", branch)

    def test_clean_stack_has_no_duplicates(self):
        git(self.repo, "switch", "-q", "-c", "lower")
        commit(self.repo, "lower1")
        git(self.repo, "switch", "-q", "-c", "upper")
        commit(self.repo, "upper1")
        self.push_ref("lower")
        self.push_ref("upper")
        self.assertEqual(pr_status.duplicate_commits("lower", "upper", self.repo), [])

    def test_base_behind_without_overlap_is_not_a_duplicate(self):
        git(self.repo, "switch", "-q", "-c", "feature")
        commit(self.repo, "feat1")
        git(self.repo, "switch", "-q", "main")
        commit(self.repo, "main2")
        self.push_ref("main")
        self.push_ref("feature")
        self.assertEqual(pr_status.duplicate_commits("main", "feature", self.repo), [])

    def test_rebased_base_makes_old_copies_duplicates(self):
        git(self.repo, "switch", "-q", "-c", "lower")
        old_lower = commit(self.repo, "lower1")
        git(self.repo, "switch", "-q", "-c", "upper")
        own = commit(self.repo, "upper1")
        # lower is rebased onto a new main commit: same patch, new SHA
        git(self.repo, "switch", "-q", "main")
        commit(self.repo, "main2")
        git(self.repo, "switch", "-q", "lower")
        git(self.repo, "rebase", "-q", "main")
        self.assertNotEqual(git(self.repo, "rev-parse", "lower"), old_lower)
        for b in ("lower", "upper"):
            self.push_ref(b)
        dups = pr_status.duplicate_commits("lower", "upper", self.repo)
        self.assertEqual(dups, [old_lower])
        self.assertNotIn(own, dups)

    def test_missing_ref_returns_empty(self):
        self.assertEqual(pr_status.duplicate_commits("nope", "also-nope", self.repo), [])


class SelectNextActionTest(unittest.TestCase):
    base = dict(
        conflicting=False, duplicates=[], gatekeeper=None,
        has_comments=False, failing=[], pending=[],
    )

    def action(self, **overrides):
        return pr_status.select_next_action(**{**self.base, **overrides})

    def test_priority_order(self):
        everything = dict(
            conflicting=True, duplicates=["abc"], gatekeeper={"awaitingWorker": True},
            has_comments=True, failing=["ci"], pending=["ci"],
        )
        expected = [
            ("conflicting", "resolve-conflict"),
            ("duplicates", "restack"),
            ("gatekeeper", "address-gatekeeper"),
            ("has_comments", "triage-comments"),
            ("failing", "fix-ci"),
            ("pending", "wait-ci"),
        ]
        for key, action in expected:
            self.assertEqual(self.action(**everything), action)
            everything[key] = self.base[key]
        self.assertEqual(self.action(**everything), "none")


if __name__ == "__main__":
    unittest.main()
