#!/usr/bin/env python3
"""Tests for skills freshness check."""
from __future__ import annotations

import subprocess
import unittest
from pathlib import Path
import sys
from unittest import mock

SCRIPTS = Path(__file__).resolve().parent
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import skills_freshness  # noqa: E402


class SkillsFreshnessTests(unittest.TestCase):
    def test_fresh_when_diff_is_quiet(self) -> None:
        calls: list[list[str]] = []

        def fake_run(cmd, **kwargs):
            calls.append(cmd)
            if cmd[:3] == ["git", "diff", "--quiet"]:
                return subprocess.CompletedProcess(cmd, 0, "", "")
            if cmd[:3] == ["git", "fetch", "origin"]:
                return subprocess.CompletedProcess(cmd, 0, "", "")
            if cmd[:3] == ["git", "rev-parse", "--show-toplevel"]:
                return subprocess.CompletedProcess(cmd, 0, "/repo\n", "")
            raise AssertionError(cmd)

        with mock.patch.object(skills_freshness.subprocess, "run", side_effect=fake_run):
            ok, msg = skills_freshness.check_skills_fresh("dev/0.9.x", fetch=True)
        self.assertTrue(ok)
        self.assertIn("match", msg)
        diff_calls = [c for c in calls if c[:3] == ["git", "diff", "--quiet"]]
        self.assertEqual(
            diff_calls[0],
            ["git", "diff", "--quiet", "HEAD", "origin/dev/0.9.x", "--", ".agents/skills"],
        )

    def test_stale_when_diff_differs(self) -> None:
        def fake_run(cmd, **kwargs):
            if cmd[:3] == ["git", "diff", "--quiet"]:
                return subprocess.CompletedProcess(cmd, 1, "", "")
            if cmd[:3] == ["git", "fetch", "origin"]:
                return subprocess.CompletedProcess(cmd, 0, "", "")
            if cmd[:3] == ["git", "rev-parse", "--show-toplevel"]:
                return subprocess.CompletedProcess(cmd, 0, "/repo\n", "")
            raise AssertionError(cmd)

        with mock.patch.object(skills_freshness.subprocess, "run", side_effect=fake_run):
            ok, msg = skills_freshness.check_skills_fresh("dev/0.9.x", fetch=False)
        self.assertFalse(ok)
        self.assertIn("differ", msg)


if __name__ == "__main__":
    unittest.main()
