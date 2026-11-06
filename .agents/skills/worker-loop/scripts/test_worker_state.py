#!/usr/bin/env python3
"""Tests for per-worker paths, shared claims, and session check-ins."""
import json
import multiprocessing
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

import worker_state


def _acquire_in_child(
    root: str,
    started: multiprocessing.Queue,
    release: multiprocessing.Queue,
) -> None:
    try:
        holder = os.getpid()
        with patch.dict(os.environ, {"LOOP_STATE_DIR": root}, clear=False):
            worker = worker_state.acquire_checkin(None, holder=holder)
        started.put(worker)
        # Stay alive until the parent has collected every check-in. The other
        # child probes this PID during its own acquire; exiting first would
        # look like a dead holder on Windows.
        release.get()
    except Exception as e:
        started.put(e)


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

    def test_prune_drops_closed_issue_and_pr(self):
        worker_state.claim("worker", "issue:1", self.claims)
        worker_state.claim("worker-2", "issue:99", self.claims)
        worker_state.claim("worker-3", "pr:2", self.claims)
        n = worker_state.prune_closed_claims([1], [2], self.claims)
        self.assertEqual(n, 1)
        claims = worker_state.load_claims(self.claims)
        self.assertEqual(set(claims), {"issue:1", "pr:2"})

    def test_prune_keeps_open_refs(self):
        worker_state.claim("worker", "issue:7", self.claims)
        worker_state.claim("worker", "pr:5", self.claims)
        self.assertEqual(worker_state.prune_closed_claims([7], [5], self.claims), 0)

    def test_prune_drops_other_workers_closed_ref(self):
        worker_state.claim("worker-2", "pr:9", self.claims)
        self.assertEqual(worker_state.prune_closed_claims([], [], self.claims), 1)
        self.assertEqual(worker_state.load_claims(self.claims), {})

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

    def test_read_json_tolerates_utf8_bom(self):
        payload = {"scheduler": {"kind": "scheduled_task", "id": "abc"}, "skipped": {"2390": "blocked"}}
        self.claims.write_bytes(b"\xef\xbb\xbf" + json.dumps(payload).encode("utf-8"))
        self.assertEqual(worker_state.read_json(self.claims), payload)

    def test_read_json_raises_on_corrupt_existing_file(self):
        # #2612: an existing file we cannot parse must raise, not return {} —
        # otherwise the next save silently wipes it.
        self.claims.write_text('{"current": "pr:1", broken', encoding="utf-8")
        with self.assertRaises(worker_state.StateFileCorrupt):
            worker_state.read_json(self.claims)

    def test_read_json_raises_on_non_object_json(self):
        self.claims.write_text("[1, 2]", encoding="utf-8")
        with self.assertRaises(worker_state.StateFileCorrupt):
            worker_state.read_json(self.claims)

    def test_read_json_missing_file_is_empty(self):
        self.assertEqual(worker_state.read_json(self.claims), {})

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
        self.holder = os.getpid()

    def test_explicit_name_is_respected(self):
        with patch.dict(os.environ, {"LOOP_STATE_DIR": str(self.root)}, clear=False):
            self.assertEqual(
                worker_state.acquire_checkin("worker-3", self.checkins, holder=self.holder),
                "worker-3",
            )
            data = worker_state.load_checkins(self.checkins)
            self.assertIn("worker-3", data)
            self.assertEqual(data["worker-3"]["pid"], self.holder)

    def test_release_frees_slot(self):
        with patch.dict(os.environ, {"LOOP_STATE_DIR": str(self.root)}, clear=False):
            worker_state.acquire_checkin("worker", self.checkins, holder=self.holder)
            self.assertTrue(worker_state.release_checkin("worker", self.checkins, holder=self.holder))
            self.assertEqual(worker_state.load_checkins(self.checkins), {})

    def test_holder_dead_frees_slot(self):
        stale = worker_state.checkin_record(999999)
        worker_state.atomic_write_json(self.checkins, {"worker": stale})
        with patch.dict(os.environ, {"LOOP_STATE_DIR": str(self.root)}, clear=False):
            self.assertEqual(
                worker_state.acquire_checkin(None, self.checkins, holder=self.holder),
                "worker",
            )
            self.assertEqual(worker_state.load_checkins(self.checkins)["worker"]["pid"], self.holder)

    def test_holder_alive_keeps_slot(self):
        alive = {100, 200}

        def _alive(pid: int | None) -> bool:
            return bool(pid) and int(pid) in alive

        with patch.dict(os.environ, {"LOOP_STATE_DIR": str(self.root)}, clear=False), patch.object(
            worker_state, "_pid_alive", side_effect=_alive
        ):
            self.assertEqual(worker_state.acquire_checkin(None, self.checkins, holder=100), "worker")
            record = worker_state.load_checkins(self.checkins)["worker"]
            self.assertFalse(worker_state.is_checkin_stale(record))
            self.assertEqual(worker_state.acquire_checkin(None, self.checkins, holder=200), "worker-2")

    def test_concurrent_checkins_get_different_slots(self):
        ctx = multiprocessing.get_context("spawn")
        started: multiprocessing.Queue = ctx.Queue()
        release: multiprocessing.Queue = ctx.Queue()
        root = str(self.root)
        processes = [
            ctx.Process(target=_acquire_in_child, args=(root, started, release))
            for _ in range(2)
        ]
        for proc in processes:
            proc.start()
        try:
            results = [started.get(timeout=15) for _ in range(2)]
        finally:
            for _ in processes:
                release.put(None)
            for proc in processes:
                proc.join(timeout=15)
        for proc in processes:
            self.assertEqual(proc.exitcode, 0)
        for item in results:
            self.assertIsInstance(item, str)
        self.assertNotEqual(results[0], results[1])

    @unittest.skipUnless(sys.platform == "win32", "Windows OpenProcess semantics only")
    def test_win_open_process_error_distinguishes_dead_from_denied(self):
        import ctypes

        cases = (
            (87, False),  # ERROR_INVALID_PARAMETER: PID does not exist
            (5, True),  # ERROR_ACCESS_DENIED: process exists, query refused
        )
        for code, expect_alive in cases:
            with self.subTest(last_error=code):
                kernel32 = MagicMock()
                kernel32.OpenProcess.return_value = 0
                with patch.object(ctypes, "WinDLL", return_value=kernel32), patch.object(
                    ctypes, "get_last_error", return_value=code
                ):
                    self.assertEqual(worker_state._win_pid_alive(4242), expect_alive)
                kernel32.OpenProcess.assert_called_once()
                kernel32.GetExitCodeProcess.assert_not_called()
                kernel32.CloseHandle.assert_not_called()

    def test_resolve_holder_pid_explicit(self):
        self.assertEqual(worker_state.resolve_holder_pid(4242), 4242)

    def test_libragent_chain_skips_bash_c_and_returns_app_pid(self):
        chain = {
            501: worker_state.ProcessInfo("python3", ["python3", "tick_gate.py", "--lock"], 502),
            502: worker_state.ProcessInfo("bash", ["bash", "-c", "python3 tick_gate.py --lock"], 19679),
            19679: worker_state.ProcessInfo(
                "libragent",
                ["./src-tauri/target/release/libragent"],
                1,
            ),
        }

        def lookup(pid: int) -> worker_state.ProcessInfo | None:
            return chain.get(pid)

        with patch.object(worker_state, "_process_info_lookup", lookup), patch(
            "os.getppid", return_value=501
        ):
            self.assertEqual(worker_state.resolve_holder_pid(), 19679)

    def test_cursor_agent_chain_falls_back_to_ttl(self):
        chain = {
            601: worker_state.ProcessInfo("python3", ["python3", "tick_gate.py"], 602),
            602: worker_state.ProcessInfo(
                "node",
                [
                    "/home/me/.local/share/cursor-agent/versions/2026.03.20-abc/node",
                    "run",
                    "shell",
                ],
                603,
            ),
            603: worker_state.ProcessInfo("node", ["node", "cursor-agent"], 1),
        }

        def lookup(pid: int) -> worker_state.ProcessInfo | None:
            return chain.get(pid)

        with patch.object(worker_state, "_process_info_lookup", lookup), patch(
            "os.getppid", return_value=601
        ):
            self.assertIsNone(worker_state.resolve_holder_pid())

    def test_same_libragent_pid_can_hold_two_auto_slots(self):
        def _alive(pid: int | None) -> bool:
            return pid == 19679

        with patch.dict(os.environ, {"LOOP_STATE_DIR": str(self.root)}, clear=False), patch.object(
            worker_state, "_pid_alive", side_effect=_alive
        ):
            self.assertEqual(
                worker_state.acquire_checkin(None, self.checkins, holder=19679),
                "worker",
            )
            self.assertEqual(
                worker_state.acquire_checkin(None, self.checkins, holder=19679),
                "worker-2",
            )

    def test_ttl_checkin_has_no_pid_field(self):
        with patch.dict(os.environ, {"LOOP_STATE_DIR": str(self.root)}, clear=False):
            worker_state.acquire_checkin(None, self.checkins, holder=None)
            record = worker_state.load_checkins(self.checkins)["worker"]
            self.assertNotIn("pid", record)
            self.assertTrue(worker_state.release_checkin("worker", self.checkins, holder=None))


if __name__ == "__main__":
    unittest.main()
