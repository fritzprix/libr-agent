#!/usr/bin/env python3
"""Tests for tick_gate logic."""
import io
import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import MagicMock, patch

import issue_candidates
import tick_gate
import worker_slots


def approvals_issue(number, labels=(), assignees=()):
    return {
        "number": number,
        "labels": [{"name": name} for name in labels],
        "assignees": [{"login": login} for login in assignees],
    }


class TickGateTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.tmp_dir = Path(self._tmp.name)
        self.state_file = self.tmp_dir / "state.json"
        self.lock_file = self.tmp_dir / "tick.lock"
        self._loop_env = patch.dict(
            os.environ,
            {"LOOP_STATE_DIR": str(self.tmp_dir), tick_gate.skills_freshness.SKIP_GATE_ENV: "1"},
        )
        self._loop_env.start()
        self.addCleanup(self._loop_env.stop)
        for name in ("WORKER_ID", "WORKER_LOOP_SESSION_ID", "CLAUDE_CODE_SESSION_ID", "WORKER_LOOP_HOLDER_PID",
                     "WORKER_LOOP_TOKEN", "WORKER_LOOP_INHERITED_TOKEN"):
            os.environ.pop(name, None)
        no_processes = patch.object(tick_gate.worker_state, "_process_info_lookup", lambda pid: None)
        no_processes.start()
        self.addCleanup(no_processes.stop)
        # list_open_prs uses issue_candidates' own gh wrapper, which patching
        # tick_gate.gh does not cover. Unpatched, a test reads the real repo.
        no_open_prs = patch.object(issue_candidates, "list_open_prs", return_value=[])
        no_open_prs.start()
        self.addCleanup(no_open_prs.stop)

    def _run(self, *args):
        """Run tick_gate.main; return (exit code, stdout)."""
        buffer = io.StringIO()
        with redirect_stdout(buffer), redirect_stderr(buffer), \
             patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), *args]), \
             self.assertRaises(SystemExit) as cm:
            tick_gate.main()
        return cm.exception.code, buffer.getvalue()

    def _lock(self):
        """--lock; return (exit code, token)."""
        code, out = self._run("--lock")
        token = next((line.split()[1] for line in out.splitlines() if line.startswith("TOKEN ")), None)
        return code, token

    def test_load_and_save_state(self):
        state = {"current": None, "lastFingerprint": "abc"}
        tick_gate.save_state(self.state_file, state)
        loaded = tick_gate.load_state(self.state_file)
        self.assertEqual(loaded, state)

    def test_corrupt_state_json_is_never_overwritten_by_lock(self):
        # #2613 owner note: a corrupt existing state.json must make --lock fail
        # hard, byte-identical, instead of silently rewriting it as {}.
        corrupt = '{"current": "pr:1", broken'
        self.state_file.write_text(corrupt, encoding="utf-8")
        buffer = io.StringIO()
        with redirect_stderr(buffer), \
             patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), "--lock"]), \
             self.assertRaises(SystemExit) as cm:
            tick_gate.main()
        self.assertEqual(cm.exception.code, tick_gate.EXIT_ERROR)
        self.assertIn("state file error", buffer.getvalue())
        self.assertEqual(self.state_file.read_bytes(), corrupt.encode("utf-8"))
        self.assertFalse(self.lock_file.is_file())

    def test_lock_acquire_and_release(self):
        self.assertFalse(tick_gate.check_lock(self.lock_file))
        tick_gate.acquire_lock(self.lock_file)
        self.assertTrue(tick_gate.check_lock(self.lock_file))
        tick_gate.release_lock(self.lock_file)
        self.assertFalse(tick_gate.check_lock(self.lock_file))

    def test_lock_with_pid(self):
        # Current process PID is alive
        tick_gate.acquire_lock(self.lock_file, pid=os.getpid())
        self.assertTrue(tick_gate.check_lock(self.lock_file))

    def test_dead_pid_is_a_stale_lock(self):
        tick_gate.acquire_lock(self.lock_file, pid=9999999)
        self.assertFalse(tick_gate.check_lock(self.lock_file))

    def test_stale_lock_by_ttl(self):
        self.assertEqual(tick_gate.LOCK_TTL_SECONDS, 1800)
        # Create a lock file with timestamp older than TTL
        old_time = time.time() - (tick_gate.LOCK_TTL_SECONDS + 10)
        self.lock_file.write_text(json.dumps({"time": old_time, "pid": os.getpid()}))
        self.assertFalse(tick_gate.check_lock(self.lock_file))

    def test_gh_failure_raises_gh_error(self):
        with patch("tick_gate.subprocess.run") as mock_run:
            mock_run.return_value = MagicMock(returncode=1, stderr="Bad credentials", stdout="")
            with self.assertRaises(tick_gate.GhError):
                tick_gate.gh("api", "user")

    def test_compute_fingerprint_graphql(self):
        fake_graphql = {
            "data": {
                "search": {
                    "nodes": [
                        {
                            "number": 1,
                            "headRefOid": "sha1",
                            "mergeable": "MERGEABLE",
                            "updatedAt": "2026-01-01T00:00:00Z",
                            "isDraft": False,
                            "author": {"login": "me"},
                            "commits": {"nodes": [{"commit": {"statusCheckRollup": {"state": "PENDING"}}}]},
                            "comments": {"nodes": [{"createdAt": "2026-01-01T01:00:00Z"}]},
                        }
                    ]
                },
                "repository": {
                    "issues": {
                        "nodes": [
                            {"number": 10, "updatedAt": "2026-01-01T00:00:00Z"}
                        ]
                    }
                }
            }
        }
        with patch.object(tick_gate, "gh") as mock_gh:
            mock_gh.side_effect = [
                json.dumps({"owner": {"login": "owner"}, "name": "repo"}),  # repo view
                "me\n",  # api user
                json.dumps(fake_graphql),  # graphql
            ]
            fp1 = tick_gate.compute_fingerprint()
            self.assertTrue(isinstance(fp1, str) and len(fp1) == 64)

            # Change in CI state produces a different fingerprint
            fake_graphql_ci_failed = json.loads(json.dumps(fake_graphql))
            fake_graphql_ci_failed["data"]["search"]["nodes"][0]["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["state"] = "FAILURE"

            mock_gh.side_effect = [
                json.dumps({"owner": {"login": "owner"}, "name": "repo"}),
                "me\n",
                json.dumps(fake_graphql_ci_failed),
            ]
            fp2 = tick_gate.compute_fingerprint()
            self.assertNotEqual(fp1, fp2)

    def _create_mock_gh(self, graphql_responses, issues=(), prs=()):
        responses = list(graphql_responses)
        idx = [0]

        def _gh(*args):
            if args == ("repo", "view", "--json", "owner,name"):
                return json.dumps({"owner": {"login": "owner"}, "name": "repo"})
            if args == ("api", "user", "--jq", ".login"):
                return "me\n"
            if len(args) >= 2 and args[0] == "api" and args[1] == "graphql":
                resp = responses[idx[0]]
                idx[0] += 1
                return json.dumps(resp)
            if args[0] == "api" and args[1].endswith("/dependencies/blocked_by"):
                raise tick_gate.GhError("gh: Not Found (HTTP 404)")
            if args[:3] == ("issue", "list", "--state"):
                return json.dumps(list(issues))
            if args[:3] == ("pr", "list", "--state"):
                return json.dumps(list(prs))
            raise ValueError(f"Unexpected gh call: {args}")

        return _gh

    IDLE = {"data": {"search": {"nodes": []}, "repository": {"issues": {"nodes": [
        {"number": 10, "updatedAt": "2026-01-01T00:00:00Z"}]}}}}
    MOVED = {"data": {"search": {"nodes": []}, "repository": {"issues": {"nodes": [
        {"number": 10, "updatedAt": "2026-01-01T02:00:00Z"}]}}}}

    def test_unchanged_fingerprint_still_runs_when_an_approved_issue_is_free(self):
        # An approved issue nobody has started must not be ignored just because the
        # tracked snapshot did not move.
        mock_gh = self._create_mock_gh([self.IDLE, self.IDLE], issues=[approvals_issue(12, ["approved"])])
        with patch.object(tick_gate, "gh", side_effect=mock_gh), \
             patch.object(tick_gate.worker_claims, "load_claims", return_value={}):
            code, token = self._lock()
            self.assertEqual(code, tick_gate.EXIT_RUN)
            self.assertIsNotNone(tick_gate.load_state(self.state_file).get("tickStartFingerprint"))
            self.assertEqual(self._run("--token", token, "--update", "--unlock")[0], tick_gate.EXIT_RUN)
            code, out = self._run("--lock")
            self.assertEqual(code, tick_gate.EXIT_RUN)
            self.assertIn("#12", out)

    def _hold_with_state(self, state):
        """Take the lease, then set state as the tick would."""
        with patch.object(tick_gate, "gh", side_effect=self._create_mock_gh([self.IDLE])):
            _, token = self._lock()
        tick_gate.save_state(self.state_file, state)
        return token

    def test_update_takes_precedence_over_current(self):
        token = self._hold_with_state(
            {"current": "some-feature-branch", "lastFingerprint": "old", "tickStartFingerprint": "new_fp_1234567890"}
        )
        self.assertEqual(self._run("--token", token, "--update")[0], tick_gate.EXIT_RUN)
        saved = tick_gate.load_state(self.state_file)
        self.assertEqual(saved.get("lastFingerprint"), "new_fp_1234567890")
        self.assertNotIn("tickStartFingerprint", saved)

    def test_update_missing_tick_start_fingerprint(self):
        token = self._hold_with_state({"current": None, "lastFingerprint": "old_fp"})
        self.assertEqual(self._run("--token", token, "--update")[0], tick_gate.EXIT_RUN)
        saved = tick_gate.load_state(self.state_file)
        self.assertEqual(saved.get("lastFingerprint"), "old_fp")
        self.assertNotIn("tickStartFingerprint", saved)

    def test_update_and_unlock_combined(self):
        token = self._hold_with_state({"current": None, "lastFingerprint": "old_fp", "tickStartFingerprint": "new"})
        self.assertTrue(tick_gate.check_lock(self.lock_file))
        self.assertEqual(self._run("--token", token, "--update", "--unlock")[0], tick_gate.EXIT_RUN)
        saved = tick_gate.load_state(self.state_file)
        self.assertEqual(saved.get("lastFingerprint"), "new")
        self.assertFalse(tick_gate.check_lock(self.lock_file))

    def test_unlock_error_still_releases_the_lock(self):
        token = self._hold_with_state({})
        with patch.object(tick_gate, "load_state", side_effect=tick_gate.GhError("simulated gh failure")):
            code, _ = self._run("--token", token, "--update", "--unlock")
        self.assertEqual(code, tick_gate.EXIT_ERROR)
        self.assertFalse(tick_gate.check_lock(self.lock_file))

    def test_changes_during_tick_detected_next_tick(self):
        mock_gh = self._create_mock_gh([self.IDLE, self.MOVED])
        with patch.object(tick_gate, "gh", side_effect=mock_gh):
            code, token = self._lock()
            self.assertEqual(code, tick_gate.EXIT_RUN)
            fp_start = tick_gate.load_state(self.state_file).get("tickStartFingerprint")
            self.assertIsNotNone(fp_start)
            self.assertTrue(tick_gate.check_lock(self.lock_file))
            self.assertEqual(self._run("--token", token, "--update", "--unlock")[0], tick_gate.EXIT_RUN)
            saved = tick_gate.load_state(self.state_file)
            self.assertEqual(saved.get("lastFingerprint"), fp_start)
            self.assertFalse(tick_gate.check_lock(self.lock_file))
            self.assertEqual(self._lock()[0], tick_gate.EXIT_RUN)
            self.assertNotEqual(tick_gate.load_state(self.state_file).get("tickStartFingerprint"), fp_start)

    def test_idle_tick_unchanged_snapshot_gives_noop(self):
        mock_gh = self._create_mock_gh([self.IDLE, self.IDLE])
        with patch.object(tick_gate, "gh", side_effect=mock_gh), \
             patch.object(tick_gate.worker_claims, "load_claims", return_value={}):
            _, token = self._lock()
            self.assertEqual(self._run("--token", token, "--update", "--unlock")[0], tick_gate.EXIT_RUN)
            self.assertEqual(self._run("--lock")[0], tick_gate.EXIT_NOOP)

    def test_work_tick_unlock_only_next_gate_runs_even_if_unchanged(self):
        mock_gh = self._create_mock_gh([self.IDLE, self.IDLE])
        with patch.object(tick_gate, "gh", side_effect=mock_gh):
            _, token = self._lock()
            self.assertEqual(self._run("--token", token, "--unlock")[0], tick_gate.EXIT_RUN)
            saved = tick_gate.load_state(self.state_file)
            self.assertIsNone(saved.get("lastFingerprint"))
            self.assertNotIn("tickStartFingerprint", saved)
            self.assertEqual(self._run("--lock")[0], tick_gate.EXIT_RUN)

    # --- skills-freshness gate (setUp skips it; these tests re-enable it) ---

    def _no_freshness_skip(self):
        # patch.dict in setUp restores the original env on cleanup, so popping
        # here only affects this test.
        os.environ.pop(tick_gate.skills_freshness.SKIP_GATE_ENV, None)

    def test_lock_exits_skills_stale_without_taking_a_slot(self):
        # G1.1: stale skills exit 5 before a slot or lease is taken.
        self._no_freshness_skip()
        stale = (False, "skills under .agents/skills differ from origin/dev/0.9.x")
        with patch.object(tick_gate.skills_freshness, "resolve_base_branch", return_value="dev/0.9.x"), \
             patch.object(tick_gate.skills_freshness, "check_skills_fresh", return_value=stale):
            code, out = self._run("--lock")
        self.assertEqual(code, tick_gate.skills_freshness.GATE_STALE_EXIT)
        self.assertIn("differ", out)
        self.assertNotIn("TOKEN", out)
        self.assertEqual(worker_slots.load_slots(), {})

    def test_lock_errors_when_dev_base_unresolvable(self):
        # G1.2: an unresolvable dev base is EXIT_ERROR, never a stale verdict.
        self._no_freshness_skip()
        error = tick_gate.skills_freshness.SkillsFreshnessError("could not resolve the dev base branch")
        with patch.object(tick_gate.skills_freshness, "resolve_base_branch", side_effect=error):
            code, out = self._run("--lock")
        self.assertEqual(code, tick_gate.EXIT_ERROR)
        self.assertIn("dev base", out)
        self.assertNotIn("TOKEN", out)
        self.assertEqual(worker_slots.load_slots(), {})

    def test_lock_errors_when_git_fetch_fails(self):
        # G1.3: a fetch failure inside the check is EXIT_ERROR, not stale (5).
        self._no_freshness_skip()
        error = tick_gate.skills_freshness.SkillsFreshnessError("git fetch origin dev/0.9.x failed")
        with patch.object(tick_gate.skills_freshness, "resolve_base_branch", return_value="dev/0.9.x"), \
             patch.object(tick_gate.skills_freshness, "check_skills_fresh", side_effect=error):
            code, out = self._run("--lock")
        self.assertEqual(code, tick_gate.EXIT_ERROR)
        self.assertIn("git fetch", out)
        self.assertNotIn("TOKEN", out)
        self.assertEqual(worker_slots.load_slots(), {})

    def test_lock_errors_when_repo_root_fails(self):
        # G1.3: CalledProcessError from _repo_root must be a clean EXIT_ERROR.
        self._no_freshness_skip()
        root_error = subprocess.CalledProcessError(128, ["git", "rev-parse", "--show-toplevel"])
        with patch.object(tick_gate.skills_freshness, "_repo_root", side_effect=root_error):
            code, out = self._run("--lock")
        self.assertEqual(code, tick_gate.EXIT_ERROR)
        self.assertIn("skills freshness check error", out)
        self.assertNotIn("TOKEN", out)
        self.assertEqual(worker_slots.load_slots(), {})


class PendingApprovedIssueTest(unittest.TestCase):
    def pending(self, issues, prs=(), claims=None, worker="worker", comments=(), open_issues=()):
        def _gh(*args):
            if args[:2] == ("api", "user"):
                return "me\n"
            if args[:3] == ("issue", "list", "--state"):
                self.assertIn("--label", args)
                self.assertEqual(args[args.index("--label") + 1], "approved")
                return json.dumps(list(issues))
            if args[:2] == ("issue", "view"):
                return json.dumps({"comments": list(comments)})
            if args[0] == "api" and args[1].endswith("/dependencies/blocked_by"):
                raise tick_gate.GhError("gh: Not Found (HTTP 404)")
            if args[0] == "api":
                number = int(args[1].rsplit("/", 1)[1])
                return "open\n" if number in open_issues else "closed\n"
            raise ValueError(f"Unexpected gh call: {args}")

        # `list_open_prs` lives in issue_candidates and uses its own gh wrapper.
        with patch.object(tick_gate, "gh", side_effect=_gh), \
             patch.object(issue_candidates, "list_open_prs", return_value=list(prs)), \
             patch.object(tick_gate.worker_claims, "load_claims", return_value=claims or {}):
            return tick_gate.pending_approved_issue(worker)

    def test_returns_none_without_an_approved_issue(self):
        self.assertIsNone(self.pending([approvals_issue(1, ["bug"]), approvals_issue(2, ["auto:producer", "validated"])]))

    def test_returns_the_lowest_approved_issue_number(self):
        self.assertEqual(self.pending([approvals_issue(9, ["approved"]), approvals_issue(3, ["approved"])]), 3)

    def test_issue_covered_by_an_open_pr_is_skipped(self):
        prs = [{"number": 7, "title": "Fixes #3", "body": "", "headRefName": "fix/slug-3"}]
        self.assertEqual(self.pending([approvals_issue(3, ["approved"]), approvals_issue(5, ["approved"])], prs=prs), 5)

    def test_issue_claimed_by_another_worker_is_skipped(self):
        claims = {"issue:3": {"worker": "worker-2", "at": 0}}
        self.assertEqual(
            self.pending([approvals_issue(3, ["approved"]), approvals_issue(5, ["approved"])], claims=claims), 5
        )

    def test_issue_assigned_to_someone_else_is_skipped(self):
        self.assertIsNone(self.pending([approvals_issue(3, ["approved"], ["other"])]))

    def test_epic_and_auto_e2e_are_not_pending(self):
        issues = [
            approvals_issue(3, ["approved", "epic"]),
            approvals_issue(5, ["approved", "auto:e2e"]),
            approvals_issue(7, ["approved"]),
        ]
        self.assertEqual(self.pending(issues), 7)
        self.assertIsNone(self.pending(issues[:2]))

    def test_issue_waiting_on_the_owner_is_not_pending(self):
        question = {"author": {"login": "me"}, "body": "<!-- worker-loop: needs-owner -->\n(a) or (b)?",
                    "createdAt": "2026-01-01T00:00:00Z"}
        issue = approvals_issue(3, ["approved", "needs-owner"])
        self.assertIsNone(self.pending([issue], comments=[question]))
        answer = {"author": {"login": "me"}, "body": "(a)", "createdAt": "2026-01-02T00:00:00Z"}
        self.assertEqual(self.pending([issue], comments=[question, answer]), 3)

    def test_issue_with_an_open_dependency_is_not_pending(self):
        blocked = {**approvals_issue(3, ["approved"]), "body": "Depends on #2"}
        self.assertIsNone(self.pending([blocked], open_issues={2}))
        self.assertEqual(self.pending([blocked]), 3)


if __name__ == "__main__":
    unittest.main()
