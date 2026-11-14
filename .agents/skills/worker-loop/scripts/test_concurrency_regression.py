#!/usr/bin/env python3
"""Regression tests for concurrent worker loops.

Outcomes here do not depend on timing: lease ages are set by editing records,
races assert only what must hold for every interleaving, and nothing is retried.
Real processes and real git repositories are used where the behaviour lives there.
"""
import multiprocessing
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

import lease
import tick_gate
import tick_lock
import worker_claims
import worker_slots
import worker_state
import worker_worktree
from worker_slots import SlotBusy
from worker_state import Owner

SCRIPTS = Path(__file__).resolve().parent
_IDENTITY_ENVS = (
    "WORKER_ID", "WORKER_LOOP_SESSION_ID", "CLAUDE_CODE_SESSION_ID", "WORKER_LOOP_HOLDER_PID",
    "WORKER_LOOP_TOKEN", "WORKER_LOOP_INHERITED_TOKEN",
)


def anon(name="a"):
    return Owner(f"anon:{name}", "anon")


def session(name="a"):
    return Owner(f"session:{name}", "session")


def live_session(name="a"):
    """A session whose process (this test) is alive."""
    return Owner(f"session:{name}", "session", os.getpid(), None, True)


class StateDirCase(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name).resolve()
        env = patch.dict(os.environ, {
            "LOOP_STATE_DIR": str(self.root),
            # The skills-freshness gate runs real git; these tests are about
            # leases and slots, so the gate is skipped here (its own behavior
            # is covered in test_tick_gate.py).
            tick_gate.skills_freshness.SKIP_GATE_ENV: "1",
        })
        env.start()
        self.addCleanup(env.stop)
        for name in _IDENTITY_ENVS:
            os.environ.pop(name, None)
        lookup = patch.object(worker_state, "_process_info_lookup", lambda pid: None)
        lookup.start()
        self.addCleanup(lookup.stop)

    def age(self, worker, seconds):
        """Make a lease look `seconds` older: slot and tick lock alike."""
        path = worker_slots.slots_path()
        slots = worker_state.read_json(path)
        slots[worker]["startedAt"] -= seconds
        slots[worker]["renewedAt"] -= seconds
        worker_state.atomic_write_json(path, slots)
        lock = lease.lock_path(worker)
        record = tick_lock.read(lock)
        record["time"] -= seconds
        record["renewedAt"] -= seconds
        worker_state.atomic_write_json(lock, record)

    def set_state(self, worker, state):
        worker_state.atomic_write_json(worker_state.state_path(worker), state)

    def gate(self, *args, env=None):
        """tick_gate.py as a real process; return (exit code, stdout)."""
        full_env = {**os.environ, "PYTHONDONTWRITEBYTECODE": "1", **(env or {})}
        result = subprocess.run(
            [sys.executable, str(SCRIPTS / "tick_gate.py"), *args],
            capture_output=True, text=True, env=full_env,
        )
        return result.returncode, result.stdout + result.stderr


class LongRunningTickTest(StateDirCase):
    def test_same_session_cannot_start_a_second_tick(self):
        held = lease.acquire(session())
        with self.assertRaises(SlotBusy) as cm:
            lease.acquire(session())
        self.assertEqual(cm.exception.reason, "same-session")
        self.assertEqual(set(worker_slots.load_slots()), {held.worker})

    def test_same_session_past_the_ttl_replaces_its_own_abandoned_tick(self):
        old = lease.acquire(session())
        self.age(old.worker, worker_state.LEASE_TTL_SECONDS + 60)
        new = lease.acquire(session())
        self.assertEqual(new.worker, old.worker)
        self.assertNotEqual(new.token, old.token)
        self.assertFalse(lease.renew(old.worker, old.token))

    def test_live_session_past_the_ttl_replaces_its_own_abandoned_tick(self):
        # Its lock is live at any age, so it must not be refused and strand the slot.
        old = lease.acquire(live_session())
        self.age(old.worker, worker_state.LEASE_TTL_SECONDS + 60)
        new = lease.acquire(live_session())
        self.assertEqual(new.worker, old.worker)
        self.assertEqual(set(worker_slots.load_slots()), {old.worker})
        self.assertTrue(lease.is_held_by(new.worker, new.token))
        self.assertFalse(lease.renew(old.worker, old.token))

    def test_live_session_recovers_a_lock_it_stranded_without_a_slot(self):
        old = lease.acquire(live_session())
        worker_slots.release_slot(old.worker, old.token)
        new = lease.acquire(live_session(), requested=old.worker)
        self.assertTrue(lease.is_held_by(old.worker, new.token))

    def test_another_live_sessions_lock_is_not_replaced(self):
        lock = lease.lock_path("worker")
        token = tick_lock.acquire(lock, live_session("other"))
        self.assertIsNone(tick_lock.acquire(lock, live_session("me")))
        self.assertTrue(tick_lock.holds(lock, token))

    def test_shared_app_owner_does_not_replace_a_running_lock(self):
        # Several ticks of one app share this ID, so it cannot prove abandonment.
        app = Owner("app:1", "app", os.getpid(), None, True)
        lock = lease.lock_path("worker")
        token = tick_lock.acquire(lock, app)
        self.assertIsNone(tick_lock.acquire(lock, app))
        self.assertTrue(tick_lock.holds(lock, token))

    def test_live_session_keeps_its_slot_past_the_ttl(self):
        running = lease.acquire(live_session("long"))
        self.age(running.worker, 7 * 24 * 3600)
        other = lease.acquire(anon("other"))
        self.assertNotEqual(other.worker, running.worker)
        self.assertTrue(lease.renew(running.worker, running.token))

    def test_renewal_keeps_an_anonymous_tick_past_the_original_ttl(self):
        running = lease.acquire(anon("long"))
        self.age(running.worker, worker_state.LEASE_TTL_SECONDS - 60)
        self.assertTrue(lease.renew(running.worker, running.token))
        self.age(running.worker, 120)  # 31 minutes after start, 2 after renewal
        self.assertNotEqual(lease.acquire(anon("other")).worker, running.worker)

    def test_unrenewed_anonymous_tick_is_taken_over(self):
        running = lease.acquire(anon("gone"))
        self.age(running.worker, worker_state.LEASE_TTL_SECONDS + 60)
        self.assertEqual(lease.acquire(anon("next")).worker, running.worker)
        self.assertFalse(lease.renew(running.worker, running.token))

    def test_reused_pid_does_not_keep_a_lease(self):
        reused = Owner("session:old", "session", os.getpid(), "not-this-process-start-time", True)
        held = lease.acquire(reused)
        self.assertEqual(lease.acquire(anon("next")).worker, held.worker)

    def test_busy_reason_names_the_actual_conflict(self):
        lease.acquire(live_session("a"), requested="worker-5")
        with self.assertRaises(SlotBusy) as cm:
            lease.acquire(anon("b"), requested="worker-5")
        self.assertEqual(cm.exception.reason, "slot-held")

    def test_session_can_run_two_workers_only_by_pinning(self):
        self.assertEqual(lease.acquire(session(), requested="worker").worker, "worker")
        self.assertEqual(lease.acquire(session(), requested="worker-2").worker, "worker-2")


class LateReleaseTest(StateDirCase):
    def takeover(self):
        old = lease.acquire(anon("old"))
        self.age(old.worker, worker_state.LEASE_TTL_SECONDS + 60)
        return old, lease.acquire(anon("new"))

    def test_old_owner_cannot_unlock_the_new_owner(self):
        old, new = self.takeover()
        worker_claims.claim(new.worker, "pr:7")
        self.set_state(new.worker, {"tickStartFingerprint": "new-owners"})
        code, out = self.gate("--worker", old.worker, "--token", old.token, "--update", "--unlock")
        self.assertEqual(code, tick_gate.EXIT_LEASE_LOST)
        self.assertIn("LEASE LOST", out)
        self.assertTrue(lease.is_held_by(new.worker, new.token))
        self.assertIn("pr:7", worker_claims.load_claims())
        self.assertEqual(worker_state.read_json(worker_state.state_path(new.worker)), {"tickStartFingerprint": "new-owners"})

    def test_old_owner_cannot_renew_or_release_primitives(self):
        old, new = self.takeover()
        lock = lease.lock_path(new.worker)
        self.assertFalse(tick_lock.renew(lock, old.token))
        self.assertFalse(tick_lock.release(lock, old.token))
        self.assertFalse(worker_slots.release_slot(new.worker, old.token))
        lease.release(old.worker, old.token)
        self.assertTrue(lease.is_held_by(new.worker, new.token))

    def test_unlock_without_a_token_changes_nothing(self):
        held = lease.acquire(anon())
        code, _ = self.gate("--worker", held.worker, "--unlock")
        self.assertEqual(code, tick_gate.EXIT_LEASE_LOST)
        self.assertTrue(lease.is_held_by(held.worker, held.token))

    def test_renew_reports_a_lost_lease(self):
        old, new = self.takeover()
        self.assertEqual(self.gate("--worker", old.worker, "--token", old.token, "--renew")[0], tick_gate.EXIT_LEASE_LOST)
        self.assertEqual(self.gate("--worker", new.worker, "--token", new.token, "--renew")[0], tick_gate.EXIT_RUN)

    def test_claim_with_a_lost_token_is_refused(self):
        old, new = self.takeover()
        result = subprocess.run(
            [sys.executable, str(SCRIPTS / "worker_state.py"), "claim",
             "--worker", old.worker, "--token", old.token, "--ref", "issue:1"],
            capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, worker_state.EXIT_LEASE_LOST)
        self.assertEqual(worker_claims.load_claims(), {})

    def test_force_unlock_is_the_explicit_operator_path(self):
        held = lease.acquire(live_session())
        self.assertEqual(self.gate("--worker", held.worker, "--force-unlock")[0], 0)
        self.assertFalse(lease.is_held_by(held.worker, held.token))


def _acquire_in_process(root, requested, name, barrier, results):
    os.environ["LOOP_STATE_DIR"] = root
    for env_name in _IDENTITY_ENVS:
        os.environ.pop(env_name, None)
    barrier.wait()
    try:
        held = lease.acquire(Owner(f"anon:{name}", "anon"), requested=requested)
        results.put(("held", held.worker, held.token))
    except SlotBusy as e:
        results.put(("busy", e.reason, None))


class ConcurrentAcquireTest(StateDirCase):
    def race(self, requested, count=8):
        ctx = multiprocessing.get_context("spawn")
        barrier, results = ctx.Barrier(count), ctx.Queue()
        procs = [
            ctx.Process(target=_acquire_in_process, args=(str(self.root), requested, str(i), barrier, results))
            for i in range(count)
        ]
        for proc in procs:
            proc.start()
        outcomes = [results.get(timeout=60) for _ in procs]
        for proc in procs:
            proc.join(timeout=60)
            self.assertEqual(proc.exitcode, 0)
        return outcomes

    def test_one_winner_for_a_pinned_slot(self):
        outcomes = self.race("worker")
        winners = [o for o in outcomes if o[0] == "held"]
        self.assertEqual(len(winners), 1)
        self.assertEqual({o[1] for o in outcomes if o[0] == "busy"}, {"slot-held"})
        self.assertTrue(lease.is_held_by("worker", winners[0][2]))

    def test_auto_assignment_gives_every_tick_its_own_slot(self):
        outcomes = self.race(None)
        self.assertEqual({o[0] for o in outcomes}, {"held"})
        self.assertEqual(len({o[1] for o in outcomes}), len(outcomes))

    def test_one_winner_for_one_claim(self):
        ctx = multiprocessing.get_context("spawn")
        barrier, results = ctx.Barrier(8), ctx.Queue()
        procs = [
            ctx.Process(target=_claim_in_process, args=(str(self.root), f"w{i}", barrier, results))
            for i in range(8)
        ]
        for proc in procs:
            proc.start()
        outcomes = [results.get(timeout=60) for _ in procs]
        for proc in procs:
            proc.join(timeout=60)
        self.assertEqual(sum(1 for holder in outcomes if holder is None), 1)

    def test_a_crashed_mutex_holder_blocks_nobody(self):
        code = (
            "import sys,os,time; sys.path.insert(0, sys.argv[1]);"
            "import worker_state; from pathlib import Path;"
            "m=worker_state.exclusive(Path(sys.argv[2])); m.__enter__();"
            "print('held',flush=True); os._exit(9)"
        )
        lock = self.root / "m.lock"
        out = subprocess.run([sys.executable, "-c", code, str(SCRIPTS), str(lock)], capture_output=True, text=True)
        self.assertEqual((out.returncode, out.stdout.strip()), (9, "held"))
        with worker_state.exclusive(lock, timeout=1.0):
            pass


def _claim_in_process(root, worker, barrier, results):
    os.environ["LOOP_STATE_DIR"] = root
    barrier.wait()
    results.put(worker_claims.claim(worker, "issue:1"))


class ErrorExitTest(StateDirCase):
    def _stub_gh(self):
        bin_dir = self.root / "stubbin"
        bin_dir.mkdir()
        if sys.platform == "win32":
            (bin_dir / "gh.cmd").write_text("@exit /b 1\r\n")
        else:
            gh = bin_dir / "gh"
            gh.write_text("#!/bin/sh\nexit 1\n")
            gh.chmod(0o755)
        # On Windows CreateProcess never executes the .cmd stub, so the real gh
        # runs there; invalid tokens make it fail the same way the stub does.
        return {
            "PATH": f"{bin_dir}{os.pathsep}{os.environ['PATH']}",
            "GH_TOKEN": "invalid-token-for-test",
            "GITHUB_TOKEN": "invalid-token-for-test",
        }

    def assert_nothing_held(self):
        self.assertEqual(worker_slots.load_slots(), {})
        self.assertFalse(tick_lock.is_held(lease.lock_path("worker")))

    def test_github_failure_releases_slot_and_lock(self):
        code, out = self.gate("--lock", env=self._stub_gh())
        self.assertEqual(code, tick_gate.EXIT_ERROR, out)
        self.assert_nothing_held()

    def test_corrupt_state_releases_slot_and_lock(self):
        path = worker_state.state_path("worker")
        path.parent.mkdir(parents=True)
        path.write_text("{broken")
        code, _ = self.gate("--lock", "--worker", "worker")
        self.assertEqual(code, tick_gate.EXIT_ERROR)
        self.assertEqual(path.read_text(), "{broken")
        self.assert_nothing_held()

    def test_corrupt_claims_releases_the_lease_before_any_work(self):
        worker_claims.claims_path().write_text("{broken")
        with self.assertRaises(worker_state.StateFileCorrupt):
            lease.acquire(anon())
        self.assert_nothing_held()

    def test_noop_releases_slot_and_lock(self):
        with patch.object(tick_gate, "compute_fingerprint", return_value="same"), \
             patch.object(tick_gate, "pending_approved_issue", return_value=None), \
             patch("sys.argv", ["tick_gate.py", "--lock"]), self.assertRaises(SystemExit) as cm:
            self.set_state("worker", {"lastFingerprint": "same"})
            tick_gate.main()
        self.assertEqual(cm.exception.code, tick_gate.EXIT_NOOP)
        self.assert_nothing_held()

    def test_unexpected_exception_after_lock_releases(self):
        with patch.object(tick_gate, "compute_fingerprint", side_effect=KeyError("boom")), \
             patch("sys.argv", ["tick_gate.py", "--lock"]), self.assertRaises(SystemExit) as cm:
            tick_gate.main()
        self.assertEqual(cm.exception.code, tick_gate.EXIT_ERROR)
        self.assert_nothing_held()

    def test_corrupt_claims_is_not_reported_as_held(self):
        worker_claims.claims_path().write_text("{broken")
        result = subprocess.run(
            [sys.executable, str(SCRIPTS / "worker_state.py"), "claim", "--worker", "w", "--ref", "issue:1"],
            capture_output=True, text=True,
        )
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertEqual(worker_claims.claims_path().read_text(), "{broken")


class ClaimRecoveryTest(StateDirCase):
    def test_malformed_claims_do_not_stop_a_tick(self):
        worker_state.atomic_write_json(worker_claims.claims_path(), {
            "issue:1": {"at": 1},
            "issue:2": "not-an-object",
            "issue:3": {"worker": "../outside"},
        })
        held = lease.acquire(anon())
        self.assertTrue(lease.is_held_by(held.worker, held.token))
        self.assertEqual(worker_claims.load_claims(), {})

    def test_unlock_releases_claims_except_current(self):
        held = lease.acquire(anon())
        worker_claims.claim(held.worker, "pr:1")
        worker_claims.claim(held.worker, "issue:2")
        self.set_state(held.worker, {"current": {"ref": "issue:2", "branch": "fix/x-2"}})
        code, _ = self.gate("--worker", held.worker, "--token", held.token, "--unlock")
        self.assertEqual(code, 0)
        self.assertEqual(set(worker_claims.load_claims()), {"issue:2"})

    def test_pr_conversion_frees_the_issue(self):
        held = lease.acquire(anon())
        worker_claims.claim(held.worker, "issue:2")
        self.set_state(held.worker, {"current": None})  # the PR is open; current was cleared
        self.gate("--worker", held.worker, "--token", held.token, "--unlock")
        self.assertEqual(worker_claims.load_claims(), {})

    def test_crashed_workers_claim_is_recovered_at_the_next_tick(self):
        crashed = lease.acquire(anon("crashed"), requested="worker-2")
        worker_claims.claim(crashed.worker, "pr:9")
        self.age(crashed.worker, worker_state.LEASE_TTL_SECONDS + 60)
        lease.acquire(anon("solo"))
        self.assertEqual(worker_claims.load_claims(), {})

    def test_live_workers_claim_is_never_taken(self):
        running = lease.acquire(live_session("running"), requested="worker-2")
        worker_claims.claim(running.worker, "pr:9")
        self.age(running.worker, 7 * 24 * 3600)
        other = lease.acquire(anon("other"))
        self.assertEqual(worker_claims.claim(other.worker, "pr:9"), "worker-2")

    def test_abandoned_current_keeps_its_claim_and_is_resumed_first(self):
        self.set_state("worker-3", {"current": {"ref": "issue:4", "branch": "fix/x-4"}})
        worker_claims.claim("worker-3", "issue:4")
        worker_claims.claim("worker-3", "pr:5")  # current does not need this one
        held = lease.acquire(anon("solo"))
        self.assertEqual(held.worker, "worker-3")
        # The slot's new owner holds both until its tick ends; then only `current` stays.
        code, _ = self.gate("--worker", held.worker, "--token", held.token, "--unlock")
        self.assertEqual(code, 0)
        self.assertEqual(set(worker_claims.load_claims()), {"issue:4"})

    def test_unreadable_state_keeps_the_claim(self):
        path = worker_state.state_path("worker-2")
        path.parent.mkdir(parents=True)
        path.write_text("{broken")
        worker_claims.claim("worker-2", "issue:4")
        lease.acquire(anon("solo"))
        self.assertIn("issue:4", worker_claims.load_claims())


class StatusTest(StateDirCase):
    def test_status_finds_every_worker(self):
        running = lease.acquire(live_session("a"), requested="worker-4")
        worker_claims.claim(running.worker, "pr:1")
        self.set_state("worker-9", {"current": {"ref": "issue:2"}})
        status = worker_state.status()
        self.assertEqual(set(status["workers"]), {"worker-4", "worker-9"})
        self.assertTrue(status["workers"]["worker-4"]["tickRunning"])
        self.assertEqual(status["workers"]["worker-4"]["claims"], ["pr:1"])
        self.assertFalse(status["workers"]["worker-9"]["slotLive"])
        self.assertEqual(status["workers"]["worker-9"]["current"], {"ref": "issue:2"})

    def test_questions_are_shared_and_asked_once(self):
        self.assertTrue(worker_state.add_needs_user("issue:1", "which?"))
        self.assertFalse(worker_state.add_needs_user("issue:1", "which?"))
        self.assertEqual(len(worker_state.status()["needsUser"]), 1)
        worker_state.clear_needs_user("issue:1")
        self.assertTrue(worker_state.add_needs_user("issue:1", "again?"))

    def test_ci_attempts_count_across_workers(self):
        self.assertEqual(worker_state.count_ci_attempt(5), 1)
        self.assertEqual(worker_state.count_ci_attempt(5), 2)

    def test_stop_finds_every_schedule(self):
        worker_state.add_scheduler("process", "1")
        worker_state.add_scheduler("scheduled_task", "b")
        self.assertEqual(len(worker_state.clear_schedulers()), 2)
        self.assertEqual(worker_state.list_schedulers(), [])

    def test_existing_per_slot_state_is_adopted_once(self):
        self.set_state("worker", {
            "scheduler": {"kind": "scheduled_task", "id": "x"},
            "ciAttempts": {"7": 2},
            "needsUser": [{"ref": "issue:1", "question": "q", "since": "s"}],
        })
        self.set_state("worker-2", {"ciAttempts": {"7": 1}})
        self.assertEqual(worker_state.count_ci_attempt(7), 3)
        self.assertEqual(worker_state.list_schedulers(), [{"kind": "scheduled_task", "id": "x"}])
        self.assertFalse(worker_state.add_needs_user("issue:1", "q"))


def _git(cwd, *args):
    return subprocess.run(
        ["git", "-c", "user.name=t", "-c", "user.email=t@example.invalid", "-c", "commit.gpgsign=false", *args],
        cwd=str(cwd), check=True, capture_output=True, text=True,
    ).stdout.strip()


class WorktreeIsolationTest(StateDirCase):
    def setUp(self):
        super().setUp()
        self.remote = self.root / "remote.git"
        _git(self.root, "init", "-q", "--bare", "-b", "main", str(self.remote))
        self.main = self.root / "repo"
        _git(self.root, "clone", "-q", str(self.remote), str(self.main))
        (self.main / "file.txt").write_text("base\n")
        _git(self.main, "add", "file.txt")
        _git(self.main, "commit", "-qm", "base")
        _git(self.main, "push", "-q", "origin", "HEAD:refs/heads/main")
        _git(self.main, "push", "-q", "origin", "HEAD:refs/heads/fix/a-1")
        _git(self.main, "fetch", "-q", "origin")
        self.trees = self.root / "repo.worktrees"
        for name in ("worker", "worker-2"):
            _git(self.main, "worktree", "add", "-q", "--detach", str(self.trees / name), "origin/main")

    def wt(self, name):
        return self.trees / name

    def run_tool(self, *args):
        with patch("sys.stdout") as out:
            lines = []
            out.write.side_effect = lines.append
            code = worker_worktree.main(list(args))
        return code, "".join(lines)

    def reset(self, held, wt=None):
        return self.run_tool("reset", "--worker", held.worker, "--token", held.token, "--wt", str(wt or self.wt(held.worker)))

    def switch(self, held, branch="fix/a-1"):
        return self.run_tool("switch", "--worker", held.worker, "--token", held.token,
                             "--wt", str(self.wt(held.worker)), "--branch", branch)

    def test_lost_lease_cannot_reset_the_new_owners_changes(self):
        old = lease.acquire(anon("old"), requested="worker")
        self.age("worker", worker_state.LEASE_TTL_SECONDS + 60)
        lease.acquire(anon("new"), requested="worker")
        edited = self.wt("worker") / "file.txt"
        edited.write_text("new owner's uncommitted work\n")
        code, out = self.reset(old)
        self.assertEqual(code, worker_state.EXIT_LEASE_LOST)
        self.assertIn("LEASE LOST", out)
        self.assertEqual(edited.read_text(), "new owner's uncommitted work\n")

    def test_token_of_one_slot_cannot_reset_another_slots_worktree(self):
        held = lease.acquire(anon(), requested="worker")
        edited = self.wt("worker-2") / "file.txt"
        edited.write_text("other run's work\n")
        code, _ = self.reset(held, wt=self.wt("worker-2"))
        self.assertEqual(code, worker_worktree.EXIT_ERROR)
        self.assertEqual(edited.read_text(), "other run's work\n")

    def test_reset_saves_an_earlier_runs_changes_before_discarding(self):
        held = lease.acquire(anon(), requested="worker")
        wt = self.wt("worker")
        (wt / "file.txt").write_text("left behind\n")
        (wt / "untracked.txt").write_text("also left behind\n")
        code, out = self.reset(held)
        self.assertEqual(code, 0, out)
        self.assertIn("RESCUED", out)
        self.assertEqual((wt / "file.txt").read_text(), "base\n")
        self.assertFalse((wt / "untracked.txt").exists())
        _git(wt, "stash", "apply", "--index", out.split()[1])
        self.assertEqual((wt / "file.txt").read_text(), "left behind\n")
        self.assertEqual((wt / "untracked.txt").read_text(), "also left behind\n")

    def test_reset_recovers_from_an_abandoned_rebase(self):
        held = lease.acquire(anon(), requested="worker")
        wt = self.wt("worker")
        _git(wt, "switch", "-q", "-c", "side")
        (wt / "file.txt").write_text("side\n")
        _git(wt, "commit", "-qam", "side")
        _git(wt, "switch", "-q", "--detach", "origin/main")
        (wt / "file.txt").write_text("other\n")
        _git(wt, "commit", "-qam", "other")
        with self.assertRaises(subprocess.CalledProcessError):
            _git(wt, "rebase", "side")
        code, out = self.reset(held)
        self.assertEqual(code, 0, out)
        self.assertEqual(_git(wt, "status", "--porcelain"), "")

    def test_reset_never_touches_current_work(self):
        held = lease.acquire(anon(), requested="worker")
        self.set_state("worker", {"current": {"ref": "issue:1"}})
        edited = self.wt("worker") / "file.txt"
        edited.write_text("work in progress\n")
        code, out = self.reset(held)
        self.assertEqual(code, 0)
        self.assertIn("READY", out)
        self.assertEqual(edited.read_text(), "work in progress\n")

    def test_switch_keeps_a_ref_to_unpushed_commits(self):
        wt = self.wt("worker")
        _git(wt, "switch", "-q", "-C", "fix/a-1", "origin/fix/a-1")
        (wt / "file.txt").write_text("never pushed\n")
        _git(wt, "commit", "-qam", "unpushed")
        unpushed = _git(wt, "rev-parse", "HEAD")
        _git(wt, "switch", "-q", "--detach")
        held = lease.acquire(anon(), requested="worker")
        code, out = self.switch(held)
        self.assertEqual(code, 0, out)
        self.assertEqual(_git(wt, "rev-parse", "HEAD"), _git(wt, "rev-parse", "origin/fix/a-1"))
        ref = f"refs/worker-loop/rescued/fix/a-1/{unpushed}"
        self.assertIn(ref, out)
        self.assertEqual(_git(wt, "rev-parse", ref), unpushed)

    def test_switch_saves_nothing_when_everything_is_pushed(self):
        _git(self.wt("worker"), "switch", "-q", "-C", "fix/a-1", "origin/fix/a-1")
        _git(self.wt("worker"), "switch", "-q", "--detach")
        code, out = self.switch(lease.acquire(anon(), requested="worker"))
        self.assertEqual(code, 0, out)
        self.assertNotIn("saved", out)
        self.assertEqual(_git(self.main, "for-each-ref", "refs/worker-loop"), "")

    def test_branch_of_a_live_run_is_not_taken(self):
        other = lease.acquire(live_session("other"), requested="worker-2")
        _git(self.wt(other.worker), "switch", "-q", "-C", "fix/a-1", "origin/fix/a-1")
        me = lease.acquire(anon("me"), requested="worker")
        code, out = self.switch(me)
        self.assertEqual(code, worker_worktree.EXIT_BRANCH_BUSY)
        self.assertIn("BRANCH BUSY", out)
        self.assertEqual(_git(self.wt(other.worker), "branch", "--show-current"), "fix/a-1")

    def test_branch_reserved_by_an_abandoned_current_is_not_taken(self):
        _git(self.wt("worker-2"), "switch", "-q", "-C", "fix/a-1", "origin/fix/a-1")
        self.set_state("worker-2", {"current": {"ref": "issue:1", "branch": "fix/a-1"}})
        me = lease.acquire(anon("me"), requested="worker")
        self.assertEqual(self.switch(me)[0], worker_worktree.EXIT_BRANCH_BUSY)

    def test_branch_left_in_an_idle_clean_slot_is_freed(self):
        _git(self.wt("worker-2"), "switch", "-q", "-C", "fix/a-1", "origin/fix/a-1")
        me = lease.acquire(anon("me"), requested="worker")
        code, out = self.switch(me)
        self.assertEqual(code, 0, out)
        self.assertIn("SWITCHED", out)
        self.assertEqual(_git(self.wt("worker"), "branch", "--show-current"), "fix/a-1")
        self.assertEqual(_git(self.wt("worker-2"), "branch", "--show-current"), "")

    def test_branch_in_a_dirty_idle_slot_stays_there(self):
        _git(self.wt("worker-2"), "switch", "-q", "-C", "fix/a-1", "origin/fix/a-1")
        edited = self.wt("worker-2") / "file.txt"
        edited.write_text("left behind\n")
        me = lease.acquire(anon("me"), requested="worker")
        code, out = self.switch(me)
        self.assertEqual(code, 0, out)
        self.assertIn("DETACHED", out)
        self.assertEqual(edited.read_text(), "left behind\n")

    def test_branch_in_the_users_checkout_is_left_alone(self):
        _git(self.main, "switch", "-q", "-C", "fix/a-1", "origin/fix/a-1")
        me = lease.acquire(anon("me"), requested="worker")
        code, out = self.switch(me)
        self.assertEqual(code, 0, out)
        self.assertIn("DETACHED", out)
        self.assertEqual(_git(self.main, "branch", "--show-current"), "fix/a-1")
        # The printed push must work from the detached worktree.
        command = out[out.index("git push"):].rstrip(")\n").split()[2:]
        (self.wt("worker") / "file.txt").write_text("fix\n")
        _git(self.wt("worker"), "commit", "-qam", "fix")
        _git(self.wt("worker"), "push", "-q", *command)
        self.assertEqual(
            _git(self.remote, "rev-parse", "refs/heads/fix/a-1"), _git(self.wt("worker"), "rev-parse", "HEAD")
        )


class LeaseRunTest(StateDirCase):
    def lease_run(self, *args, env=None, **kwargs):
        full_env = {**os.environ, "PYTHONDONTWRITEBYTECODE": "1", **(env or {})}
        return subprocess.run(
            [sys.executable, str(SCRIPTS / "lease_run.py"), *args],
            capture_output=True, text=True, env=full_env, timeout=60, **kwargs,
        )

    def py(self, code):
        return [sys.executable, "-c", code]

    def test_lost_lease_never_starts_the_command(self):
        old = lease.acquire(anon("old"))
        self.age(old.worker, worker_state.LEASE_TTL_SECONDS + 60)
        lease.acquire(anon("new"))
        marker = self.root / "ran"
        result = self.lease_run("--worker", old.worker, "--token", old.token, "--",
                                *self.py(f"open({str(marker)!r}, 'w')"))
        self.assertEqual(result.returncode, worker_state.EXIT_LEASE_LOST)
        self.assertFalse(marker.exists())

    def test_passes_the_commands_exit_code(self):
        held = lease.acquire(anon())
        result = self.lease_run("--worker", held.worker, "--token", held.token, "--", *self.py("raise SystemExit(7)"))
        self.assertEqual(result.returncode, 7)

    def test_a_long_command_keeps_the_lease_renewed(self):
        held = lease.acquire(anon())
        self.age(held.worker, worker_state.LEASE_TTL_SECONDS - 5)
        # The command waits until it sees the renewal in the lock file, so the test
        # passes on any machine speed and fails by timeout if renewal never happens.
        lock = lease.lock_path(held.worker)
        waits_for_renewal = (
            "import json,time,sys\n"
            f"p={str(lock)!r}\n"
            # Renewal replaces the file; on Windows a read can hit it mid-replace.
            "def r():\n"
            "    while True:\n"
            "        try: return json.load(open(p))['renewedAt']\n"
            "        except (OSError, ValueError): time.sleep(0.01)\n"
            "start=r()\n"
            "while r()==start: time.sleep(0.02)\n"
        )
        result = self.lease_run("--worker", held.worker, "--token", held.token, "--interval", "0.05", "--",
                                *self.py(waits_for_renewal))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(worker_state.lease_expired(tick_lock.read(lock)))

    def test_losing_the_lease_stops_the_command(self):
        held = lease.acquire(anon())
        marker = self.root / "started"
        sleeper = f"import time; open({str(marker)!r}, 'w').close(); time.sleep(300)"
        proc = subprocess.Popen(
            [sys.executable, str(SCRIPTS / "lease_run.py"), "--worker", held.worker, "--token", held.token,
             "--interval", "0.05", "--", *self.py(sleeper)],
            env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"}, stderr=subprocess.PIPE, text=True,
        )
        deadline = time.monotonic() + 30
        while not marker.exists():
            self.assertLess(time.monotonic(), deadline, "command never started")
            time.sleep(0.02)
        self.gate("--worker", held.worker, "--force-unlock")
        new = lease.acquire(anon("new"), requested=held.worker)
        _, stderr = proc.communicate(timeout=60)
        self.assertEqual(proc.returncode, worker_state.EXIT_LEASE_LOST, stderr)
        self.assertIn("LEASE LOST", stderr)
        self.assertTrue(lease.is_held_by(new.worker, new.token))

    INNER_TICK = (
        "import os,subprocess,sys\n"
        "gate=[sys.executable, sys.argv[1]]\n"
        "a=subprocess.run(gate+['--lock'],capture_output=True,text=True)\n"
        "b=subprocess.run(gate+['--unlock'],capture_output=True,text=True)\n"
        "c=subprocess.run(gate+['--renew'],capture_output=True,text=True)\n"
        "print(os.environ['WORKER_ID'])\n"
        "print('LOCK',a.returncode,a.stdout)\n"
        "print('UNLOCK',b.returncode,b.stdout)\n"
        "print('STILL_HELD',c.returncode)\n"
        "raise SystemExit(int(sys.argv[2]))\n"
    )

    def test_wrapper_passes_its_lease_to_its_own_tick(self):
        self.set_state("worker", {"current": {"ref": "issue:1"}})  # RUN without reaching GitHub
        result = self.lease_run("--acquire", "--", *self.py(self.INNER_TICK), str(SCRIPTS / "tick_gate.py"), "5")
        self.assertEqual(result.returncode, 5, result.stdout + result.stderr)
        self.assertNotIn("BUSY", result.stdout)
        self.assertIn("LOCK 0", result.stdout)
        self.assertIn("LOCK KEPT", result.stdout)
        self.assertIn("STILL_HELD 0", result.stdout)
        # The wrapper released on the way out, even though the command failed.
        self.assertEqual(worker_slots.load_slots(), {})
        self.assertFalse(tick_lock.is_held(lease.lock_path("worker")))

    def test_wrapper_reports_busy_without_running_the_command(self):
        lease.acquire(live_session("other"), requested="worker")
        marker = self.root / "ran"
        result = self.lease_run("--acquire", "--worker", "worker", "--", *self.py(f"open({str(marker)!r}, 'w')"))
        self.assertEqual(result.returncode, 2)
        self.assertIn("reason=slot-held", result.stdout)
        self.assertFalse(marker.exists())

    def test_leftover_inherited_token_is_not_trusted(self):
        running = lease.acquire(live_session("other"), requested="worker")
        self.set_state("worker", {"current": {"ref": "issue:1"}})
        code, out = self.gate("--lock", "--worker", "worker", env={"WORKER_LOOP_INHERITED_TOKEN": "leftover"})
        self.assertEqual(code, tick_gate.EXIT_BUSY, out)
        self.assertIn("reason=slot-held", out)
        self.assertTrue(lease.is_held_by(running.worker, running.token))


if __name__ == "__main__":
    unittest.main()
