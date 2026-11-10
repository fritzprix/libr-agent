#!/usr/bin/env python3
"""Tests for paths, JSON files, the mutex, claims, slots, and session identity."""
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

import worker_claims
import worker_slots
import worker_state
from worker_state import Owner


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
        self.assertIsNone(worker_claims.claim("worker", "issue:1", self.claims))
        self.assertIsNone(worker_claims.claim("worker", "issue:1", self.claims))
        self.assertEqual(worker_claims.claim("worker-2", "issue:1", self.claims), "worker")
        self.assertIsNone(worker_claims.claim("worker-2", "issue:2", self.claims))

    def test_release_only_by_holder(self):
        worker_claims.claim("worker", "pr:5", self.claims)
        self.assertFalse(worker_claims.release("worker-2", "pr:5", self.claims))
        self.assertTrue(worker_claims.release("worker", "pr:5", self.claims))
        self.assertIsNone(worker_claims.claim("worker-2", "pr:5", self.claims))

    def test_claimed_by_other(self):
        worker_claims.claim("worker", "issue:7", self.claims)
        claims = worker_claims.load_claims(self.claims)
        self.assertIsNone(worker_claims.claimed_by_other(claims, "issue:7", "worker"))
        self.assertEqual(worker_claims.claimed_by_other(claims, "issue:7", "worker-2"), "worker")
        self.assertIsNone(worker_claims.claimed_by_other(claims, "issue:8", "worker-2"))

    def test_invalid_ref_is_rejected(self):
        with self.assertRaises(ValueError):
            worker_claims.claim("worker", "2547", self.claims)

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

    def test_leftover_lock_file_does_not_block_claims(self):
        # A kernel lock has no stale state: a crashed writer's file is harmless.
        self.claims.with_name("claims.json.lock").write_text("")
        self.assertIsNone(worker_claims.claim("worker", "issue:1", self.claims))


class SlotTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        self.slots = self.root / "checkins.json"
        env = patch.dict(os.environ, {"LOOP_STATE_DIR": str(self.root)})
        env.start()
        self.addCleanup(env.stop)

    def acquire(self, owner, token="t", requested=None):
        return worker_slots.acquire_slot(owner, token, requested, path=self.slots)

    def test_explicit_name_is_respected(self):
        owner = Owner("session:a", "session", os.getpid(), None, True)
        self.assertEqual(self.acquire(owner, requested="worker-3"), "worker-3")
        self.assertEqual(worker_slots.load_slots(self.slots)["worker-3"]["pid"], os.getpid())

    def test_release_needs_the_token(self):
        self.acquire(Owner("anon:a", "anon"))
        self.assertFalse(worker_slots.release_slot("worker", "other", self.slots))
        self.assertTrue(worker_slots.release_slot("worker", "t", self.slots))
        self.assertEqual(worker_slots.load_slots(self.slots), {})

    def test_dead_holder_frees_slot(self):
        self.acquire(Owner("session:dead", "session", 9999999, None, True))
        self.assertEqual(self.acquire(Owner("anon:b", "anon"), "u"), "worker")

    def test_live_session_process_keeps_slot(self):
        self.acquire(Owner("session:a", "session", os.getpid(), None, True))
        self.assertEqual(self.acquire(Owner("anon:b", "anon"), "u"), "worker-2")

    def test_app_process_can_hold_two_slots(self):
        app = Owner("app:7:s", "app", os.getpid(), None, False)
        self.assertEqual(self.acquire(app, "a"), "worker")
        self.assertEqual(self.acquire(app, "b"), "worker-2")

    def test_anonymous_record_has_no_pid_field(self):
        self.acquire(Owner("anon:a", "anon"))
        self.assertNotIn("pid", worker_slots.load_slots(self.slots)["worker"])

    def test_corrupt_state_blocks_only_that_slot(self):
        (self.root / "worker-2").mkdir()
        (self.root / "worker-2" / "state.json").write_text("{broken")
        self.assertEqual(self.acquire(Owner("anon:a", "anon")), "worker")


class HolderResolutionTest(unittest.TestCase):
    def resolve(self, chain, start):
        with patch.object(worker_state, "_process_info_lookup", chain.get), patch("os.getppid", return_value=start), \
                patch.dict(os.environ):
            os.environ.pop("WORKER_LOOP_HOLDER_PID", None)
            return worker_state.resolve_holder()

    def test_libragent_is_an_app_holder(self):
        chain = {
            501: worker_state.ProcessInfo("python3", ["python3", "tick_gate.py", "--lock"], 502),
            502: worker_state.ProcessInfo("bash", ["bash", "-c", "python3 tick_gate.py --lock"], 19679),
            19679: worker_state.ProcessInfo("libragent", ["./src-tauri/target/release/libragent"], 1),
        }
        self.assertEqual(self.resolve(chain, 501), (19679, "app"))

    def test_claude_code_is_a_session_holder(self):
        chain = {
            7061: worker_state.ProcessInfo("bash", ["/bin/bash", "-c", "source snapshot"], 5396),
            5396: worker_state.ProcessInfo("claude", ["claude"], 4995),
            4995: worker_state.ProcessInfo("bash", ["bash"], 1),
        }
        self.assertEqual(self.resolve(chain, 7061), (5396, "session"))

    def test_npm_claude_code_is_a_session_holder(self):
        chain = {
            10: worker_state.ProcessInfo("bash", ["bash", "-c", "x"], 11),
            11: worker_state.ProcessInfo("node", ["node", "/usr/local/bin/claude"], 1),
        }
        self.assertEqual(self.resolve(chain, 10), (11, "session"))

    def test_cursor_agent_has_no_holder(self):
        chain = {
            601: worker_state.ProcessInfo("python3", ["python3", "tick_gate.py"], 602),
            602: worker_state.ProcessInfo("node", ["/x/cursor-agent/node", "run", "shell"], 603),
            603: worker_state.ProcessInfo("node", ["node", "cursor-agent"], 1),
        }
        self.assertEqual(self.resolve(chain, 601), (None, None))

    def test_session_id_wins_and_survives_processes(self):
        with patch.object(worker_state, "_process_info_lookup", lambda pid: None), \
                patch.dict(os.environ, {"WORKER_LOOP_SESSION_ID": "abc"}):
            first = worker_state.resolve_owner()
            second = worker_state.resolve_owner()
        self.assertEqual((first.id, first.scope), ("session:abc", "session"))
        self.assertEqual(first.id, second.id)

    def test_without_identity_each_owner_is_unique(self):
        with patch.object(worker_state, "_process_info_lookup", lambda pid: None), patch.dict(os.environ):
            for name in ("WORKER_LOOP_SESSION_ID", "CLAUDE_CODE_SESSION_ID", "WORKER_LOOP_HOLDER_PID"):
                os.environ.pop(name, None)
            self.assertEqual(worker_state.resolve_owner().scope, "anon")
            self.assertNotEqual(worker_state.resolve_owner().id, worker_state.resolve_owner().id)

    def test_macos_process_info_comes_from_ps(self):
        values = {"comm": "/usr/local/bin/claude", "ppid": "42", "args": "claude --flag"}
        with patch.object(worker_state.sys, "platform", "darwin"), \
                patch.object(worker_state, "Path", side_effect=lambda p: type("P", (), {"exists": lambda self: False})()), \
                patch.object(worker_state, "_ps", side_effect=lambda pid, field: values[field]):
            info = worker_state._process_info(7)
        self.assertEqual((info.comm, info.ppid, info.cmdline), (values["comm"], 42, ["claude", "--flag"]))


class WindowsProbeTest(unittest.TestCase):
    def test_open_process_error_distinguishes_dead_from_denied(self):
        import ctypes

        for code, expect_alive in ((87, False), (5, True)):
            with self.subTest(last_error=code):
                kernel32 = MagicMock()
                kernel32.OpenProcess.return_value = 0
                # create=True: these attributes exist only on Windows.
                with patch.object(ctypes, "WinDLL", return_value=kernel32, create=True), patch.object(
                    ctypes, "get_last_error", return_value=code, create=True
                ):
                    self.assertEqual(worker_state._win_pid_alive(4242), expect_alive)
                kernel32.OpenProcess.assert_called_once()
                kernel32.GetExitCodeProcess.assert_not_called()
                kernel32.CloseHandle.assert_not_called()

    def test_own_start_time_is_readable_and_stable(self):
        start = worker_state._pid_start(os.getpid())
        self.assertTrue(start)
        self.assertEqual(worker_state._pid_start(os.getpid()), start)

    @unittest.skipUnless(sys.platform == "win32", "needs Windows' 100ns creation times")
    def test_windows_start_time_differs_between_processes(self):
        child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"])
        try:
            start = worker_state._pid_start(child.pid)
            self.assertTrue(start)
            self.assertNotEqual(start, worker_state._pid_start(os.getpid()))
        finally:
            child.kill()
            child.wait()


if __name__ == "__main__":
    unittest.main()
