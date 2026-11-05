#!/usr/bin/env python3
"""Tests for per-worker paths, shared claims, and session check-ins."""
import json
import os
import tempfile
import multiprocessing
import time
import unittest
from pathlib import Path
from unittest.mock import patch

import worker_state


def _acquire_in_child(root: str, out: multiprocessing.Queue) -> None:
    try:
        with patch.dict(os.environ, {"LOOP_STATE_DIR": root}, clear=False):
            out.put(worker_state.acquire_checkin(None))
    except Exception as e:
        out.put(e)


class WorkerStateTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        self.claims = self.root / "claims.json"

    def test_each_worker_has_its_own_dir(self):
        with patch.dict(os.environ, {"LOOP_STATE_DIR": str(self.root)}):
            self.assertNotEqual(worker_state.worker_dir("worker"), worker_state.worker_dir("worker-2"))
            self.assertEqual(worker_state.worker_dir("worker-2"), self.root / "worker-2")

    def test_worker_name_validation(self):
        self.assertEqual(worker_state.worker_name("worker-2"), "worker-2")
        for bad in ("../x", "Worker", "", "a b"):
            with self.assertRaises(ValueError):
                worker_state.worker_name(bad or "-")

    def test_claim_is_exclusive(self):
        self.assertIsNone(worker_state.claim("worker", "issue:1", self.claims))
        self.assertIsNone(worker_state.claim("worker", "issue:1", self.claims))
        self.assertEqual(worker_state.claim("worker-2", "issue:1", self.claims), "worker")
        self.assertIsNone(worker_state.claim("worker-2", "issue:2", self.claims))

    def test_release_only_by_holder(self):
        worker_state.claim("worker", "pr:5", self.claims)
        self.assertFalse(worker_state.release("worker-2", "pr:5", self.claims))
        self.assertTrue(worker_state.release("worker", "pr:5", self.claims))
        self.assertIsNone(worker_state.claim("worker-2", "pr:5", self.claims))

    def test_claimed_by_other(self):
        worker_state.claim("worker", "issue:7", self.claims)
        claims = worker_state.load_claims(self.claims)
        self.assertIsNone(worker_state.claimed_by_other(claims, "issue:7", "worker"))
        self.assertEqual(worker_state.claimed_by_other(claims, "issue:7", "worker-2"), "worker")
        self.assertIsNone(worker_state.claimed_by_other(claims, "issue:8", "worker-2"))

    def test_invalid_ref_is_rejected(self):
        with self.assertRaises(ValueError):
            worker_state.claim("worker", "2547", self.claims)

    def test_atomic_write_leaves_no_temp_files(self):
        worker_state.atomic_write_json(self.claims, {"a": 1})
        self.assertEqual(json.loads(self.claims.read_text(encoding="utf-8")), {"a": 1})
        self.assertEqual([p.name for p in self.root.iterdir()], ["claims.json"])

    def test_stale_claims_lock_is_broken(self):
        lock = self.claims.with_name("claims.json.lock")
        lock.write_text("")
        old = lock.stat().st_mtime - worker_state.CLAIMS_LOCK_STALE_SECONDS - 5
        os.utime(lock, (old, old))
        self.assertIsNone(worker_state.claim("worker", "issue:1", self.claims))
        self.assertFalse(lock.exists())


class CheckinTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        self.checkins = self.root / "checkins.json"

    def test_explicit_name_is_respected(self):
        with patch.dict(os.environ, {"LOOP_STATE_DIR": str(self.root)}, clear=False):
            self.assertEqual(worker_state.acquire_checkin("worker-3", self.checkins), "worker-3")
            data = worker_state.load_checkins(self.checkins)
            self.assertIn("worker-3", data)
            self.assertEqual(data["worker-3"]["pid"], os.getpid())

    def test_release_frees_slot(self):
        with patch.dict(os.environ, {"LOOP_STATE_DIR": str(self.root)}, clear=False):
            worker_state.acquire_checkin("worker", self.checkins)
            self.assertTrue(worker_state.release_checkin("worker", self.checkins))
            self.assertEqual(worker_state.load_checkins(self.checkins), {})

    def test_stale_slot_is_reused(self):
        stale = worker_state.checkin_record(pid=999999, heartbeat=int(time.time()) - 9999)
        worker_state.atomic_write_json(self.checkins, {"worker": stale})
        with patch.dict(os.environ, {"LOOP_STATE_DIR": str(self.root)}, clear=False):
            self.assertEqual(worker_state.acquire_checkin(None, self.checkins), "worker")
            self.assertEqual(worker_state.load_checkins(self.checkins)["worker"]["pid"], os.getpid())

    def test_concurrent_checkins_get_different_slots(self):
        ctx = multiprocessing.get_context("spawn")
        queue: multiprocessing.Queue = ctx.Queue()
        root = str(self.root)
        processes = [
            ctx.Process(target=_acquire_in_child, args=(root, queue))
            for _ in range(2)
        ]
        for proc in processes:
            proc.start()
        for proc in processes:
            proc.join(timeout=15)
            self.assertEqual(proc.exitcode, 0)
        results = [queue.get(timeout=5) for _ in range(2)]
        for item in results:
            self.assertIsInstance(item, str)
        self.assertNotEqual(results[0], results[1])

    def test_heartbeat_refresh(self):
        with patch.dict(os.environ, {"LOOP_STATE_DIR": str(self.root)}, clear=False):
            worker_state.acquire_checkin("worker", self.checkins)
            before = worker_state.load_checkins(self.checkins)["worker"]["heartbeat"]
            time.sleep(1.1)
            worker_state.refresh_checkin("worker", self.checkins)
            after = worker_state.load_checkins(self.checkins)["worker"]["heartbeat"]
            self.assertGreater(after, before)


if __name__ == "__main__":
    unittest.main()
