#!/usr/bin/env python3
"""Tests for producer_gate."""
import contextlib
import io
import json
import os
import tempfile
import time
import unittest
from pathlib import Path

import producer_gate

DAY = producer_gate.DEFAULT_PERIOD_SECONDS


class ProducerGateTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.state_file = Path(self._tmp.name) / "state.json"
        self.lock_file = Path(self._tmp.name) / "tick.lock"

    def run_gate(self, *argv):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = producer_gate.main(["--state-file", str(self.state_file), *argv])
        return code, out.getvalue().strip()

    def write_state(self, state):
        self.state_file.write_text(json.dumps(state), encoding="utf-8")

    def test_never_run_tasks_are_due_in_task_order(self):
        self.assertEqual(producer_gate.select_due_task({}, time.time()), "security")

    def test_most_overdue_task_wins(self):
        now = 10 * DAY
        last_runs = {task: now - DAY for task in producer_gate.TASKS}
        last_runs["uiux"] = now - 3 * DAY
        last_runs["cleanup"] = now - 2 * DAY
        self.assertEqual(producer_gate.select_due_task({"lastRun": last_runs}, now), "uiux")

    def test_noop_when_nothing_is_due(self):
        now = int(time.time())
        self.write_state({"lastRun": {task: now for task in producer_gate.TASKS}})
        self.assertEqual(self.run_gate(), (producer_gate.EXIT_NOOP, "NOOP"))

    def test_period_override_in_state(self):
        now = 10 * DAY
        state = {"lastRun": {task: now - 2 * 3600 for task in producer_gate.TASKS}}
        self.assertIsNone(producer_gate.select_due_task(state, now))
        state["periods"] = {"e2e": 3600}
        self.assertEqual(producer_gate.select_due_task(state, now), "e2e")
        self.assertEqual(producer_gate.period_seconds(state, "security"), DAY)

    def test_invalid_period_override_falls_back_to_default(self):
        for bad in (0, -5, "1h", True, None):
            self.assertEqual(producer_gate.period_seconds({"periods": {"e2e": bad}}, "e2e"), DAY)

    def test_named_task_runs_even_when_not_due(self):
        now = int(time.time())
        self.write_state({"lastRun": {task: now for task in producer_gate.TASKS}})
        self.assertEqual(self.run_gate("--task", "doc-drift"), (producer_gate.EXIT_RUN, "RUN doc-drift"))

    def test_lock_acquire_busy_and_unlock(self):
        code, out = self.run_gate("--lock", "--pid", str(os.getpid()))
        self.assertEqual((code, out), (producer_gate.EXIT_RUN, "RUN security"))
        self.assertTrue(self.lock_file.is_file())
        self.assertEqual(self.run_gate(), (producer_gate.EXIT_BUSY, "BUSY (tick in progress)"))
        self.assertEqual(self.run_gate("--unlock"), (producer_gate.EXIT_RUN, "UNLOCKED"))
        self.assertFalse(self.lock_file.exists())

    def test_stale_lock_is_ignored(self):
        old = time.time() - (producer_gate.tick_gate.LOCK_TTL_SECONDS + 10)
        self.lock_file.write_text(json.dumps({"time": old, "pid": os.getpid()}))
        self.assertEqual(self.run_gate("--lock")[1], "RUN security")

    def test_dead_pid_lock_is_ignored(self):
        self.lock_file.write_text(json.dumps({"time": int(time.time()), "pid": 9999999}))
        self.assertEqual(self.run_gate()[1], "RUN security")

    def test_done_records_last_run_and_unlocks(self):
        self.run_gate("--lock")
        code, out = self.run_gate("--done", "security", "--unlock")
        self.assertEqual(code, producer_gate.EXIT_RUN)
        self.assertIn("DONE security", out)
        self.assertFalse(self.lock_file.exists())
        state = json.loads(self.state_file.read_text(encoding="utf-8"))
        self.assertAlmostEqual(state["lastRun"]["security"], time.time(), delta=5)
        self.assertEqual(self.run_gate()[1], "RUN e2e")

    def test_state_path_uses_producer_loop_directory(self):
        self.assertEqual(producer_gate.default_state_path().parts[-2:], ("producer-loop", "state.json"))


if __name__ == "__main__":
    unittest.main()
