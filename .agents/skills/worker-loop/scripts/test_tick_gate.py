#!/usr/bin/env python3
"""Tests for tick_gate logic."""
import json
import os
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

import tick_gate


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

        # Dead PID is recognized as stale
        dead_pid = 9999999
        tick_gate.acquire_lock(self.lock_file, pid=dead_pid)
        self.assertFalse(tick_gate.check_lock(self.lock_file))

    def test_stale_lock_by_ttl(self):
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

    def test_update_takes_precedence_over_current(self):
        state = {"current": "some-feature-branch", "lastFingerprint": "old"}
        tick_gate.save_state(self.state_file, state)
        with patch.object(tick_gate, "compute_fingerprint", return_value="new_fp_1234567890"), \
             patch("sys.argv", ["tick_gate.py", "--state-file", str(self.state_file), "--update"]), \
             self.assertRaises(SystemExit) as cm:
            tick_gate.main()
        self.assertEqual(cm.exception.code, tick_gate.EXIT_RUN)
        saved = tick_gate.load_state(self.state_file)
        self.assertEqual(saved.get("lastFingerprint"), "new_fp_1234567890")


if __name__ == "__main__":
    unittest.main()
