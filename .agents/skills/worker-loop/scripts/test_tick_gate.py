#!/usr/bin/env python3
"""Tests for tick_gate logic."""
import io
import json
import os
import sys
import tempfile
import time
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import MagicMock, patch

import issue_candidates
import tick_gate


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

    def test_load_and_save_state(self):
        state = {"current": None, "lastFingerprint": "abc"}
        tick_gate.save_state(self.state_file, state)
        loaded = tick_gate.load_state(self.state_file)
        self.assertEqual(loaded, state)

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

    @unittest.skipIf(sys.platform == "win32", "PID liveness probing is POSIX-only (os.kill(pid, 0))")
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
            if args[:3] == ("issue", "list", "--state"):
                return json.dumps(list(issues))
            if args[:3] == ("pr", "list", "--state"):
                return json.dumps(list(prs))
            raise ValueError(f"Unexpected gh call: {args}")

        return _gh

    def test_unchanged_fingerprint_still_runs_when_an_approved_issue_is_free(self):
        # An approved issue nobody has started must not be ignored just because the
        # tracked snapshot did not move.
        idle_graphql = {
            "data": {
                "search": {"nodes": []},
                "repository": {"issues": {"nodes": [{"number": 10, "updatedAt": "2026-01-01T00:00:00Z"}]}},
            }
        }
        mock_gh = self._create_mock_gh(
            [idle_graphql, idle_graphql],
            issues=[approvals_issue(12, ["approved"])],
        )
        with patch.object(tick_gate, "gh", side_effect=mock_gh), \
             patch.object(tick_gate.worker_state, "load_claims", return_value={}):
            with patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), "--lock"]), \
                 self.assertRaises(SystemExit) as cm:
                tick_gate.main()
            self.assertEqual(cm.exception.code, tick_gate.EXIT_RUN)
            self.assertIsNotNone(tick_gate.load_state(self.state_file).get("tickStartFingerprint"))

            # Idle tick end persists the start-of-tick fingerprint.
            with patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), "--update", "--unlock"]), \
                 self.assertRaises(SystemExit) as cm:
                tick_gate.main()
            self.assertEqual(cm.exception.code, tick_gate.EXIT_RUN)

            buffer = io.StringIO()
            with redirect_stdout(buffer), \
                 patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), "--lock"]), \
                 self.assertRaises(SystemExit) as cm:
                tick_gate.main()
            self.assertEqual(cm.exception.code, tick_gate.EXIT_RUN)
            self.assertIn("#12", buffer.getvalue())

    def test_update_takes_precedence_over_current(self):
        state = {"current": "some-feature-branch", "lastFingerprint": "old", "tickStartFingerprint": "new_fp_1234567890"}
        tick_gate.save_state(self.state_file, state)
        with patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), "--update"]), \
             self.assertRaises(SystemExit) as cm:
            tick_gate.main()
        self.assertEqual(cm.exception.code, tick_gate.EXIT_RUN)
        saved = tick_gate.load_state(self.state_file)
        self.assertEqual(saved.get("lastFingerprint"), "new_fp_1234567890")
        self.assertNotIn("tickStartFingerprint", saved)

    def test_update_missing_tick_start_fingerprint(self):
        state = {"current": None, "lastFingerprint": "old_fp"}
        tick_gate.save_state(self.state_file, state)
        with patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), "--update"]), \
             self.assertRaises(SystemExit) as cm:
            tick_gate.main()
        self.assertEqual(cm.exception.code, tick_gate.EXIT_RUN)
        saved = tick_gate.load_state(self.state_file)
        self.assertEqual(saved.get("lastFingerprint"), "old_fp")
        self.assertNotIn("tickStartFingerprint", saved)

    def test_update_and_unlock_combined(self):
        tick_gate.acquire_lock(self.lock_file)
        self.assertTrue(tick_gate.check_lock(self.lock_file))
        state = {"current": None, "lastFingerprint": "old_fp", "tickStartFingerprint": "new_fp_combined"}
        tick_gate.save_state(self.state_file, state)

        with patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), "--update", "--unlock"]), \
             self.assertRaises(SystemExit) as cm:
            tick_gate.main()

        self.assertEqual(cm.exception.code, tick_gate.EXIT_RUN)
        saved = tick_gate.load_state(self.state_file)
        self.assertEqual(saved.get("lastFingerprint"), "new_fp_combined")
        self.assertNotIn("tickStartFingerprint", saved)
        self.assertFalse(tick_gate.check_lock(self.lock_file))

    def test_update_and_unlock_gh_error_releases_lock(self):
        tick_gate.acquire_lock(self.lock_file)
        self.assertTrue(tick_gate.check_lock(self.lock_file))

        with patch.object(tick_gate, "load_state", side_effect=tick_gate.GhError("simulated gh failure")), \
             patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), "--update", "--unlock"]), \
             self.assertRaises(SystemExit) as cm:
            tick_gate.main()

        self.assertEqual(cm.exception.code, tick_gate.EXIT_ERROR)
        self.assertEqual(cm.exception.code, 3)
        self.assertFalse(tick_gate.check_lock(self.lock_file))

    def test_changes_during_tick_detected_next_tick(self):
        # (a) changes made during a tick (fingerprint differs from tick start) are detected next tick (gate RUNs)
        fake_graphql_start = {
            "data": {
                "search": {"nodes": []},
                "repository": {"issues": {"nodes": [{"number": 10, "updatedAt": "2026-01-01T00:00:00Z"}]}},
            }
        }
        fake_graphql_during = {
            "data": {
                "search": {"nodes": []},
                "repository": {"issues": {"nodes": [{"number": 10, "updatedAt": "2026-01-01T02:00:00Z"}]}},
            }
        }
        mock_gh = self._create_mock_gh([fake_graphql_start, fake_graphql_during])
        with patch.object(tick_gate, "gh", side_effect=mock_gh):
            # Tick 1 start: gate runs with --lock
            with patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), "--lock"]), \
                 self.assertRaises(SystemExit) as cm:
                tick_gate.main()
            self.assertEqual(cm.exception.code, tick_gate.EXIT_RUN)

            state1 = tick_gate.load_state(self.state_file)
            fp_start = state1.get("tickStartFingerprint")
            self.assertIsNotNone(fp_start)
            self.assertTrue(tick_gate.check_lock(self.lock_file))

            # Tick 1 end: idle tick persists start-of-tick fingerprint and unlocks
            with patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), "--update", "--unlock"]), \
                 self.assertRaises(SystemExit) as cm:
                tick_gate.main()
            self.assertEqual(cm.exception.code, tick_gate.EXIT_RUN)

            state1_end = tick_gate.load_state(self.state_file)
            self.assertEqual(state1_end.get("lastFingerprint"), fp_start)
            self.assertNotIn("tickStartFingerprint", state1_end)
            self.assertFalse(tick_gate.check_lock(self.lock_file))

            # Tick 2 start: snapshot changed during tick 1, gate must detect change and RUN
            with patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), "--lock"]), \
                 self.assertRaises(SystemExit) as cm:
                tick_gate.main()
            self.assertEqual(cm.exception.code, tick_gate.EXIT_RUN)

            state2 = tick_gate.load_state(self.state_file)
            self.assertNotEqual(state2.get("tickStartFingerprint"), fp_start)

    def test_idle_tick_unchanged_snapshot_gives_noop(self):
        # (b) after an idle tick with --update --unlock, an unchanged snapshot gives NOOP
        #     because no `approved` issue is waiting.
        fake_graphql = {
            "data": {
                "search": {"nodes": []},
                "repository": {"issues": {"nodes": [{"number": 10, "updatedAt": "2026-01-01T00:00:00Z"}]}},
            }
        }
        mock_gh = self._create_mock_gh([fake_graphql, fake_graphql])
        with patch.object(tick_gate, "gh", side_effect=mock_gh), \
             patch.object(tick_gate.worker_state, "load_claims", return_value={}):
            # Tick 1 start: gate runs with --lock
            with patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), "--lock"]), \
                 self.assertRaises(SystemExit) as cm:
                tick_gate.main()
            self.assertEqual(cm.exception.code, tick_gate.EXIT_RUN)

            state1 = tick_gate.load_state(self.state_file)
            fp_start = state1.get("tickStartFingerprint")
            self.assertIsNotNone(fp_start)

            # Tick 1 end: idle tick with --update --unlock
            with patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), "--update", "--unlock"]), \
                 self.assertRaises(SystemExit) as cm:
                tick_gate.main()
            self.assertEqual(cm.exception.code, tick_gate.EXIT_RUN)

            state1_end = tick_gate.load_state(self.state_file)
            self.assertEqual(state1_end.get("lastFingerprint"), fp_start)
            self.assertNotIn("tickStartFingerprint", state1_end)
            self.assertFalse(tick_gate.check_lock(self.lock_file))

            # Tick 2 start: snapshot unchanged and nothing approved, gate gives NOOP (exit 1)
            with patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), "--lock"]), \
                 self.assertRaises(SystemExit) as cm:
                tick_gate.main()
            self.assertEqual(cm.exception.code, tick_gate.EXIT_NOOP)

    def test_work_tick_unlock_only_next_gate_runs_even_if_unchanged(self):
        # (c) after a work tick with --unlock only, the next gate RUNs even if snapshot unchanged
        fake_graphql = {
            "data": {
                "search": {"nodes": []},
                "repository": {"issues": {"nodes": [{"number": 10, "updatedAt": "2026-01-01T00:00:00Z"}]}},
            }
        }
        mock_gh = self._create_mock_gh([fake_graphql, fake_graphql])
        with patch.object(tick_gate, "gh", side_effect=mock_gh):
            # Tick 1 start: gate runs with --lock
            with patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), "--lock"]), \
                 self.assertRaises(SystemExit) as cm:
                tick_gate.main()
            self.assertEqual(cm.exception.code, tick_gate.EXIT_RUN)

            state1 = tick_gate.load_state(self.state_file)
            fp_start = state1.get("tickStartFingerprint")
            self.assertIsNotNone(fp_start)

            # Tick 1 end: work tick finishes with --unlock only (no --update)
            with patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), "--unlock"]), \
                 self.assertRaises(SystemExit) as cm:
                tick_gate.main()
            self.assertEqual(cm.exception.code, tick_gate.EXIT_RUN)

            state1_end = tick_gate.load_state(self.state_file)
            self.assertIsNone(state1_end.get("lastFingerprint"))
            self.assertNotIn("tickStartFingerprint", state1_end)
            self.assertFalse(tick_gate.check_lock(self.lock_file))

            # Tick 2 start: even though snapshot is unchanged, gate must RUN
            with patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), "--lock"]), \
                 self.assertRaises(SystemExit) as cm:
                tick_gate.main()
            self.assertEqual(cm.exception.code, tick_gate.EXIT_RUN)


class PendingApprovedIssueTest(unittest.TestCase):
    def pending(self, issues, prs=(), claims=None, worker="worker"):
        def _gh(*args):
            if args[:2] == ("api", "user"):
                return "me\n"
            if args[:3] == ("issue", "list", "--state"):
                return json.dumps(list(issues))
            raise ValueError(f"Unexpected gh call: {args}")

        # `list_open_prs` lives in issue_candidates and uses its own gh wrapper.
        with patch.object(tick_gate, "gh", side_effect=_gh), \
             patch.object(issue_candidates, "list_open_prs", return_value=list(prs)), \
             patch.object(tick_gate.worker_state, "load_claims", return_value=claims or {}):
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


if __name__ == "__main__":
    unittest.main()
