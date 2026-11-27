#!/usr/bin/env python3
"""Tests for the lease heartbeat (#3031).

The heartbeat is a real process: these tests start it, watch the slot record
change, and prove it stops when the lease ends or the watched process dies.
"""
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

import lease
import lease_heartbeat
import tick_lock
import worker_slots
import worker_state
from worker_state import Owner

INTERVAL = "0.2"
OWNER = Owner("app:4242:100", "app", 4242, "100", False)


def wait_for(predicate, timeout: float = 10.0) -> bool:
    deadline = time.time() + timeout
    while time.time() < deadline:
        if predicate():
            return True
        time.sleep(0.05)
    return False


class HeartbeatTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        self.env = patch.dict(
            os.environ,
            {"LOOP_STATE_DIR": str(self.root), "WORKER_LOOP_HEARTBEAT_SECONDS": INTERVAL},
        )
        self.env.start()
        self.addCleanup(self.env.stop)
        self.addCleanup(self._kill_leftovers)
        self.worker = "worker-2"
        self.token = "tok-heartbeat"
        # A lease is a slot *and* its tick lock; `lease.renew` extends both, so a
        # heartbeat over a bare slot would only ever report "lease lost".
        worker_slots.acquire_slot(OWNER, self.token, self.worker)
        self.lock = lease.lock_path(self.worker)
        self.assertTrue(tick_lock.acquire(self.lock, OWNER, self.token))
        self.addCleanup(tick_lock.release, self.lock, self.token)

    def _kill_leftovers(self):
        pid = lease_heartbeat.heartbeat_pid(self.worker)
        if pid and lease_heartbeat.running(pid):
            lease_heartbeat.stop(self.worker)

    def record(self) -> dict:
        return worker_slots.load_slots().get(self.worker) or {}

    def test_start_records_a_live_pid_on_the_slot_record(self):
        pid = lease_heartbeat.start(self.worker, self.token, os.getpid())
        self.assertIsNotNone(pid)
        self.assertEqual(self.record().get("heartbeatPid"), pid)
        self.assertTrue(lease_heartbeat.running(pid))

    def test_the_heartbeat_renews_the_slot_without_the_tick_asking(self):
        # An old renewedAt is exactly the state a quiet tick is in after 30 min.
        with worker_slots.exclusive(worker_slots.slots_path().with_name("checkins.json.lock")):
            slots = worker_slots.read_json(worker_slots.slots_path())
            slots[self.worker]["renewedAt"] = int(time.time()) - worker_state.LEASE_TTL_SECONDS + 30
            worker_state.atomic_write_json(worker_slots.slots_path(), slots)
        stale = self.record()["renewedAt"]

        lease_heartbeat.start(self.worker, self.token, os.getpid())

        self.assertTrue(
            wait_for(lambda: self.record().get("renewedAt", 0) > stale, timeout=10),
            f"heartbeat did not renew: {self.record()}",
        )

    def test_stop_kills_the_heartbeat(self):
        pid = lease_heartbeat.start(self.worker, self.token, os.getpid())
        self.assertTrue(lease_heartbeat.running(pid))
        self.assertTrue(lease_heartbeat.stop(self.worker))
        self.assertTrue(wait_for(lambda: not lease_heartbeat.running(pid), timeout=10))

    def test_it_exits_when_the_watched_process_dies(self):
        watched = subprocess.Popen(
            [sys.executable, "-c", "import time; time.sleep(60)"],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        self.addCleanup(lambda: watched.kill() if watched.poll() is None else None)
        pid = lease_heartbeat.start(self.worker, self.token, watched.pid)
        self.assertTrue(lease_heartbeat.running(pid))

        watched.kill()
        watched.wait(timeout=10)

        self.assertTrue(
            wait_for(lambda: not lease_heartbeat.running(pid), timeout=10),
            "the heartbeat kept running after the process it watches died",
        )

    def test_it_exits_when_the_lease_belongs_to_someone_else(self):
        pid = lease_heartbeat.start(self.worker, self.token, os.getpid())
        self.assertTrue(lease_heartbeat.running(pid))

        worker_slots.release_slot(self.worker, self.token)

        self.assertTrue(
            wait_for(lambda: not lease_heartbeat.running(pid), timeout=10),
            "the heartbeat kept renewing a lease that was released",
        )

    def test_run_stops_after_its_max_lifetime(self):
        """The ceiling is what finally stops an app-scoped heartbeat (#3031)."""
        started = time.time()
        code = lease_heartbeat.run(self.worker, self.token, os.getpid(), 0.1, 0.3)
        self.assertEqual(code, 0)
        self.assertLess(time.time() - started, 5)

    def test_the_first_renewal_waits_one_interval(self):
        """`acquire` just wrote a fresh lease; renewing again at once is a race.

        An immediate renewal let the heartbeat overwrite a lease that a test or
        an operator had aged on purpose, so takeover checks measured timing
        instead of liveness (#3031 G1.1).
        """
        path = worker_slots.slots_path()
        slots = worker_state.read_json(path)
        slots[self.worker]["renewedAt"] = int(time.time()) - worker_state.LEASE_TTL_SECONDS + 30
        worker_state.atomic_write_json(path, slots)
        stale = self.record()["renewedAt"]

        with patch.dict(os.environ, {"WORKER_LOOP_HEARTBEAT_SECONDS": "2"}):
            self.assertIsNotNone(lease_heartbeat.start(self.worker, self.token, os.getpid()))
            time.sleep(0.5)
            self.assertEqual(
                self.record()["renewedAt"], stale, "renewed before one interval had passed"
            )

        self.assertTrue(
            wait_for(lambda: self.record().get("renewedAt", 0) > stale, timeout=10),
            f"the heartbeat never renewed: {self.record()}",
        )

    def test_stop_never_signals_a_pid_that_was_reused(self):
        """A PID alone is not the heartbeat: a reused one is another process."""
        pid = lease_heartbeat.start(self.worker, self.token, os.getpid())
        self.assertTrue(lease_heartbeat.running(pid))

        path = worker_slots.slots_path()
        slots = worker_state.read_json(path)
        recorded = slots[self.worker].get("heartbeatStart")
        self.assertIsNotNone(recorded, "the start time must be recorded with the PID")
        slots[self.worker]["heartbeatStart"] = "some-other-process"
        worker_state.atomic_write_json(path, slots)

        self.assertFalse(
            lease_heartbeat.stop(self.worker), "it signalled a PID it did not start"
        )
        self.assertTrue(lease_heartbeat.running(pid), "the heartbeat was killed anyway")

        slots[self.worker]["heartbeatStart"] = recorded
        worker_state.atomic_write_json(path, slots)
        self.assertTrue(lease_heartbeat.stop(self.worker))


class LeaseReleaseTest(unittest.TestCase):
    """The lease itself starts and stops the heartbeat, not the skill text."""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        self.env = patch.dict(
            os.environ,
            {"LOOP_STATE_DIR": str(self.root), "WORKER_LOOP_HEARTBEAT_SECONDS": INTERVAL},
        )
        self.env.start()
        self.addCleanup(self.env.stop)

    def test_acquire_starts_one_and_release_stops_it(self):
        held = lease.acquire(OWNER, "worker-2")
        self.addCleanup(lease.release, held.worker, held.token)
        pid = lease_heartbeat.heartbeat_pid(held.worker)
        self.assertIsNotNone(pid, "lease.acquire did not start a heartbeat")
        self.assertTrue(lease_heartbeat.running(pid))

        lease.release(held.worker, held.token)

        self.assertTrue(wait_for(lambda: not lease_heartbeat.running(pid), timeout=10))
        self.assertFalse(worker_slots.holds_slot(held.worker, held.token))

    def test_a_stale_token_does_not_stop_the_new_owners_heartbeat(self):
        """`release` frees nothing for a stale token; it must also signal nothing."""
        held = lease.acquire(OWNER, "worker-2")
        self.addCleanup(lease.release, held.worker, held.token)
        pid = lease_heartbeat.heartbeat_pid(held.worker)
        self.assertIsNotNone(pid)
        self.assertTrue(lease_heartbeat.running(pid))

        lease.release(held.worker, "a-token-from-an-older-run")

        self.assertTrue(
            lease_heartbeat.running(pid), "a stale token stopped the owner's heartbeat"
        )
        self.assertTrue(worker_slots.holds_slot(held.worker, held.token))


class TokenRequiredTest(unittest.TestCase):
    """`set-current` and `clear-current` must prove they are this tick (#3031)."""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        self.env = patch.dict(os.environ, {"LOOP_STATE_DIR": str(self.root)})
        self.env.start()
        self.addCleanup(self.env.stop)
        self.script = Path(__file__).resolve().parent / "worker_state.py"

    def run_cli(self, *args) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(self.script), *args],
            capture_output=True,
            text=True,
            env={**os.environ},
        )

    def test_set_current_without_a_token_is_refused(self):
        proc = self.run_cli("set-current", "--worker", "worker-2", "--ref", "issue:7")
        self.assertEqual(proc.returncode, 2, proc.stdout + proc.stderr)
        self.assertIn("require --token", proc.stdout)
        self.assertFalse(worker_state.read_json(worker_state.state_path("worker-2")).get("current"))

    def test_clear_current_without_a_token_is_refused(self):
        proc = self.run_cli("clear-current", "--worker", "worker-2")
        self.assertEqual(proc.returncode, 2, proc.stdout + proc.stderr)
        self.assertIn("require --token", proc.stdout)

    def test_a_live_token_still_works(self):
        token = "tok-set-current"
        worker_slots.acquire_slot(OWNER, token, "worker-2")
        self.addCleanup(lease.release, "worker-2", token)
        proc = self.run_cli(
            "set-current", "--worker", "worker-2", "--token", token, "--ref", "issue:7", "--branch", "feat/x"
        )
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        self.assertEqual(
            worker_state.read_json(worker_state.state_path("worker-2")).get("current"),
            {"ref": "issue:7", "branch": "feat/x"},
        )


if __name__ == "__main__":
    unittest.main()
