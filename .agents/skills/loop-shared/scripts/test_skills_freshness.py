#!/usr/bin/env python3
"""Tests for skills freshness check."""
from __future__ import annotations

import contextlib
import io
import os
import subprocess
import tempfile
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

    def test_fetch_failure_raises_error_not_stale(self) -> None:
        # G1.3: a failed git fetch (network/auth) is an error, never "skills stale".
        def fake_run(cmd, **kwargs):
            if cmd[:3] == ["git", "fetch", "origin"]:
                return subprocess.CompletedProcess(cmd, 128, "", "unable to access origin")
            if cmd[:3] == ["git", "rev-parse", "--show-toplevel"]:
                return subprocess.CompletedProcess(cmd, 0, "/repo\n", "")
            raise AssertionError(cmd)

        with mock.patch.object(skills_freshness.subprocess, "run", side_effect=fake_run):
            with self.assertRaises(skills_freshness.SkillsFreshnessError) as ctx:
                skills_freshness.check_skills_fresh("dev/0.9.x", fetch=True)
        self.assertIn("git fetch", str(ctx.exception))

    def test_diff_unexpected_exit_code_raises_error(self) -> None:
        # G1.3: git diff exit codes other than 0 or 1 (e.g. bad revision) are errors.
        def fake_run(cmd, **kwargs):
            if cmd[:3] == ["git", "diff", "--quiet"]:
                return subprocess.CompletedProcess(cmd, 128, "", "fatal: bad revision")
            if cmd[:3] == ["git", "rev-parse", "--show-toplevel"]:
                return subprocess.CompletedProcess(cmd, 0, "/repo\n", "")
            raise AssertionError(cmd)

        with mock.patch.object(skills_freshness.subprocess, "run", side_effect=fake_run):
            with self.assertRaises(skills_freshness.SkillsFreshnessError) as ctx:
                skills_freshness.check_skills_fresh("dev/0.9.x", fetch=False)
        self.assertIn("bad revision", str(ctx.exception))

    def test_resolve_base_branch_raises_when_resolver_fails(self) -> None:
        # G1.2: no silent fallback to "main" when resolve_dev_base.py fails.
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            resolve = repo / ".agents/skills/create-pr/scripts/resolve_dev_base.py"
            resolve.parent.mkdir(parents=True)
            resolve.write_text("# stub\n", encoding="utf-8")
            failed = subprocess.CompletedProcess(["python", str(resolve)], 1, "", "no dev base")
            with mock.patch.object(skills_freshness, "_repo_root", return_value=repo), \
                 mock.patch.object(skills_freshness.subprocess, "run", return_value=failed):
                with self.assertRaises(skills_freshness.SkillsFreshnessError) as ctx:
                    skills_freshness.resolve_base_branch(None)
        self.assertIn("dev base", str(ctx.exception))

    def test_cli_exits_error_when_base_unresolvable(self) -> None:
        # G1.2: the CLI must exit 2, not run the check against a guessed branch.
        stderr = io.StringIO()
        with mock.patch.object(
            skills_freshness, "resolve_base_branch",
            side_effect=skills_freshness.SkillsFreshnessError("could not resolve the dev base branch"),
        ), contextlib.redirect_stderr(stderr):
            code = skills_freshness.main([])
        self.assertEqual(code, skills_freshness.EXIT_ERROR)
        self.assertIn("dev base", stderr.getvalue())

    def test_cli_exits_error_when_check_raises(self) -> None:
        stderr = io.StringIO()
        with mock.patch.object(skills_freshness, "resolve_base_branch", return_value="dev/0.9.x"), \
             mock.patch.object(
                 skills_freshness, "check_skills_fresh",
                 side_effect=skills_freshness.SkillsFreshnessError("git fetch origin dev/0.9.x failed"),
             ), contextlib.redirect_stderr(stderr):
            code = skills_freshness.main([])
        self.assertEqual(code, skills_freshness.EXIT_ERROR)
        self.assertIn("git fetch", stderr.getvalue())

    def _gate_with_stale_skills(self, sync_result: tuple[bool, str]) -> tuple[int | None, str]:
        stderr = io.StringIO()
        with mock.patch.dict(os.environ), \
             mock.patch.object(skills_freshness, "resolve_base_branch", return_value="dev/0.9.x"), \
             mock.patch.object(
                 skills_freshness, "check_skills_fresh",
                 return_value=(False, "skills under .agents/skills differ from origin/dev/0.9.x"),
             ), \
             mock.patch.object(skills_freshness, "fast_forward_main", return_value=sync_result), \
             contextlib.redirect_stderr(stderr):
            os.environ.pop(skills_freshness.SKIP_GATE_ENV, None)
            code = skills_freshness.gate_skills_freshness()
        return code, stderr.getvalue()

    def test_gate_helper_maps_stale_to_gate_stale_exit(self) -> None:
        code, err = self._gate_with_stale_skills((False, "checkout /repo is on 'feat/x', not dev/0.9.x"))
        self.assertEqual(code, skills_freshness.GATE_STALE_EXIT)
        self.assertIn("differ", err)
        self.assertIn("not dev/0.9.x", err)

    def test_gate_helper_continues_after_fast_forward(self) -> None:
        code, err = self._gate_with_stale_skills((True, "fast-forwarded dev/0.9.x"))
        self.assertIsNone(code)
        self.assertIn("fast-forwarded", err)

    def test_gate_helper_maps_check_error_to_gate_error_exit(self) -> None:
        # G1.3: fetch/diff errors must map to the gate's error exit, not stale.
        stderr = io.StringIO()
        with mock.patch.dict(os.environ), \
             mock.patch.object(skills_freshness, "resolve_base_branch", return_value="dev/0.9.x"), \
             mock.patch.object(
                 skills_freshness, "check_skills_fresh",
                 side_effect=skills_freshness.SkillsFreshnessError("git fetch origin dev/0.9.x failed"),
             ), contextlib.redirect_stderr(stderr):
            os.environ.pop(skills_freshness.SKIP_GATE_ENV, None)
            code = skills_freshness.gate_skills_freshness()
        self.assertEqual(code, skills_freshness.GATE_ERROR_EXIT)
        self.assertIn("git fetch", stderr.getvalue())

    def test_gate_helper_catches_repo_root_failure(self) -> None:
        # G1.3: CalledProcessError from _repo_root() must not become a traceback.
        stderr = io.StringIO()
        root_error = subprocess.CalledProcessError(128, ["git", "rev-parse", "--show-toplevel"])
        with mock.patch.dict(os.environ), \
             mock.patch.object(skills_freshness, "_repo_root", side_effect=root_error), \
             contextlib.redirect_stderr(stderr):
            os.environ.pop(skills_freshness.SKIP_GATE_ENV, None)
            code = skills_freshness.gate_skills_freshness()
        self.assertEqual(code, skills_freshness.GATE_ERROR_EXIT)
        self.assertIn("skills freshness check error", stderr.getvalue())

    def test_gate_helper_skips_when_env_set(self) -> None:
        with mock.patch.dict(os.environ, {skills_freshness.SKIP_GATE_ENV: "1"}), \
             mock.patch.object(skills_freshness, "check_skills_fresh", side_effect=AssertionError("must not run")):
            self.assertIsNone(skills_freshness.gate_skills_freshness())


BASE = "dev/9.9.x"


def _run_git(cwd: Path, *args: str) -> str:
    return subprocess.run(
        ["git", "-c", "user.name=t", "-c", "user.email=t@localhost", "-c", "commit.gpgsign=false", *args],
        cwd=cwd, capture_output=True, text=True, check=True,
    ).stdout.strip()


class FastForwardMainTests(unittest.TestCase):
    """Real git: a checkout on BASE whose origin moved one skills commit ahead."""

    def setUp(self) -> None:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        root = Path(tmp.name)
        origin = root / "origin.git"
        _run_git(root, "init", "--bare", "--quiet", str(origin))
        self.main = root / "main"
        _run_git(root, "init", "--quiet", "-b", BASE, str(self.main))
        _run_git(self.main, "remote", "add", "origin", str(origin))
        skill = self.main / ".agents/skills/x/SKILL.md"
        skill.parent.mkdir(parents=True)
        skill.write_text("v1\n", encoding="utf-8")
        _run_git(self.main, "add", ".")
        _run_git(self.main, "commit", "--quiet", "-m", "v1")
        _run_git(self.main, "push", "--quiet", "origin", BASE)

        other = root / "other"
        _run_git(root, "clone", "--quiet", "-b", BASE, str(origin), str(other))
        (other / ".agents/skills/x/SKILL.md").write_text("v2\n", encoding="utf-8")
        _run_git(other, "commit", "--quiet", "-am", "v2")
        _run_git(other, "push", "--quiet", "origin", BASE)
        _run_git(self.main, "fetch", "--quiet", "origin")

        patcher = mock.patch.object(skills_freshness, "_repo_root", return_value=self.main)
        patcher.start()
        self.addCleanup(patcher.stop)

    def _skill(self) -> str:
        return (self.main / ".agents/skills/x/SKILL.md").read_text(encoding="utf-8")

    def test_clean_checkout_on_base_is_fast_forwarded(self) -> None:
        (self.main / "notes.txt").write_text("untracked files do not block\n", encoding="utf-8")
        synced, message = skills_freshness.fast_forward_main(BASE)
        self.assertTrue(synced, message)
        self.assertEqual(self._skill(), "v2\n")
        fresh, _ = skills_freshness.check_skills_fresh(BASE, fetch=False)
        self.assertTrue(fresh)

    def test_dirty_checkout_is_left_alone(self) -> None:
        (self.main / ".agents/skills/x/SKILL.md").write_text("local edit\n", encoding="utf-8")
        synced, message = skills_freshness.fast_forward_main(BASE)
        self.assertFalse(synced)
        self.assertIn("uncommitted changes", message)
        self.assertEqual(self._skill(), "local edit\n")

    def test_diverged_checkout_is_left_alone(self) -> None:
        (self.main / "local.txt").write_text("local\n", encoding="utf-8")
        _run_git(self.main, "add", "local.txt")
        _run_git(self.main, "commit", "--quiet", "-m", "local")
        synced, message = skills_freshness.fast_forward_main(BASE)
        self.assertFalse(synced)
        self.assertIn("cannot fast-forward", message)
        self.assertEqual(self._skill(), "v1\n")

    def test_checkout_on_another_branch_is_left_alone(self) -> None:
        _run_git(self.main, "switch", "--quiet", "-c", "feat/y")
        synced, message = skills_freshness.fast_forward_main(BASE)
        self.assertFalse(synced)
        self.assertIn("is on 'feat/y'", message)
        self.assertEqual(self._skill(), "v1\n")


if __name__ == "__main__":
    unittest.main()
