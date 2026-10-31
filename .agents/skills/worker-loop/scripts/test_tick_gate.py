#!/usr/bin/env python3
"""Tests for tick_gate logic."""
import json
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

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

    def test_stale_lock_ignored(self):
        # Create a lock file with timestamp 20 minutes ago
        old_time = time.time() - 1200
        self.lock_file.write_text(json.dumps({"pid": 999999, "time": old_time}))
        self.assertFalse(tick_gate.check_lock(self.lock_file, ttl_seconds=900))

    def test_compute_fingerprint(self):
        with patch.object(tick_gate, "gh") as mock_gh:
            mock_gh.side_effect = [
                json.dumps([{"number": 1, "headRefOid": "abc", "mergeable": "MERGEABLE", "updatedAt": "2026-01-01T00:00:00Z"}]),
                json.dumps([{"number": 10, "updatedAt": "2026-01-01T00:00:00Z"}]),
            ]
            fp1 = tick_gate.compute_fingerprint()
            self.assertTrue(isinstance(fp1, str) and len(fp1) == 64)

            # Same inputs produce same hash
            mock_gh.side_effect = [
                json.dumps([{"number": 1, "headRefOid": "abc", "mergeable": "MERGEABLE", "updatedAt": "2026-01-01T00:00:00Z"}]),
                json.dumps([{"number": 10, "updatedAt": "2026-01-01T00:00:00Z"}]),
            ]
            fp2 = tick_gate.compute_fingerprint()
            self.assertEqual(fp1, fp2)


if __name__ == "__main__":
    unittest.main()
