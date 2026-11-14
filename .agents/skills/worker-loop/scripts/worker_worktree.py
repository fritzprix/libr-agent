#!/usr/bin/env python3
"""Worktree operations a tick must not run unguarded.

  worker_worktree.py reset  --worker W --token T --wt PATH
  worker_worktree.py switch --worker W --token T --wt PATH --branch B
  worker_worktree.py takeover --worker W --token T --wt PATH --from DEAD [--orphan-after SECONDS]

A slot's worktree belongs to whoever holds the slot, so each command first renews
the lease and refuses if it is lost or PATH is not this slot's worktree. `reset`
saves uncommitted changes in a stash before discarding anything, so changes left
by an earlier run stay recoverable. `switch` will not take a branch from a live
run, frees it from an abandoned, clean worktree, and keeps a ref to any local
commits that resetting the branch to the remote would drop. `takeover` resumes
an orphaned slot's `current` (see worker_slots.orphaned_slots): it replaces the
dead run's lease with a handoff lease first, so that run can never renew it and
no new tick can acquire it meanwhile, aborts any rebase or merge in progress in
the dead worktree, commits its uncommitted changes as WIP on the branch, checks
the branch out here with its local commits, and moves the claim and `current`
to this slot. If any step fails, the slot is reported as orphaned again.

Prints one result line. Exit codes:
  0 - done: READY | RESET | RESCUED <stash> | SWITCHED <branch> | DETACHED <branch> | TOOK OVER <ref>
  2 - skip this item: BRANCH BUSY <path> | NOT ORPHANED <slot> | CLAIMED <ref> by <worker>
  3 - error
  4 - LEASE LOST: stop working
"""
from __future__ import annotations

import argparse
import subprocess
import sys
import time
from pathlib import Path

import lease
import worker_claims
import worker_slots
import worker_state

EXIT_BRANCH_BUSY = 2
EXIT_ERROR = 3
EXIT_LEASE_LOST = worker_state.EXIT_LEASE_LOST


class WorktreeError(RuntimeError):
    pass


def git(wt: Path, *args: str) -> str:
    result = subprocess.run(["git", "-C", str(wt), *args], capture_output=True, text=True, encoding="utf-8")
    if result.returncode != 0:
        raise WorktreeError(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def _git_path_exists(wt: Path, name: str) -> bool:
    path = Path(git(wt, "rev-parse", "--git-path", name))
    return (path if path.is_absolute() else wt / path).exists()


def _abort_in_progress(wt: Path) -> None:
    """A dead run may have stopped mid-operation; unmerged paths cannot be stashed."""
    for marker, command in (
        ("rebase-merge", "rebase"),
        ("rebase-apply", "rebase"),
        ("MERGE_HEAD", "merge"),
        ("CHERRY_PICK_HEAD", "cherry-pick"),
    ):
        if _git_path_exists(wt, marker):
            git(wt, command, "--abort")


def _is_dirty(wt: Path) -> bool:
    return bool(git(wt, "status", "--porcelain"))


def rescue_changes(wt: Path, worker: str) -> str | None:
    """Stash everything uncommitted, ignored files excepted. Returns the stash commit."""
    _abort_in_progress(wt)
    if not _is_dirty(wt):
        return None
    message = f"worker-loop rescue: {worker} {time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())}"
    git(wt, "-c", "user.name=worker-loop", "-c", "user.email=worker-loop@localhost",
        "stash", "push", "--include-untracked", "-m", message)
    return git(wt, "rev-parse", "stash@{0}")


def reset(wt: Path, worker: str) -> str:
    current = worker_state.read_json(worker_state.state_path(worker)).get("current")
    if current:
        return "READY (current work is set: resume it, nothing was reset)"
    stash = rescue_changes(wt, worker)
    git(wt, "reset", "--hard")
    git(wt, "clean", "-fd")
    git(wt, "switch", "--detach")
    return f"RESCUED {stash}" if stash else "RESET"


def _branch_occupant(wt: Path, branch: str) -> Path | None:
    """The worktree that has `branch` checked out, per git itself."""
    path = None
    for line in git(wt, "worktree", "list", "--porcelain").splitlines():
        if line.startswith("worktree "):
            path = Path(line[len("worktree "):])
        elif line == f"branch refs/heads/{branch}":
            return path
    return None


def _occupant_is_replaceable(occupant: Path, sibling_root: Path) -> bool:
    """True only for another slot's worktree that nobody is using or will resume."""
    slot = occupant.name
    if occupant.parent.resolve() != sibling_root or not worker_slots.SLOT_NAME.match(slot):
        return False
    if slot in worker_slots.live_workers():
        return False
    return not worker_state.read_json(worker_state.state_path(slot)).get("current")


def _save_unpushed_commits(wt: Path, branch: str, remote_ref: str) -> str | None:
    """Keep a ref to local commits that resetting `branch` to the remote would drop."""
    try:
        local = git(wt, "rev-parse", "--verify", "--quiet", f"refs/heads/{branch}")
    except WorktreeError:
        return None  # no local branch yet
    if not git(wt, "rev-list", "--max-count=1", local, f"^{remote_ref}"):
        return None
    ref = f"refs/worker-loop/rescued/{branch}/{local}"
    git(wt, "update-ref", ref, local)
    return ref


def switch(wt: Path, branch: str) -> tuple[int, str]:
    remote_ref = f"origin/{branch}"
    occupant = _branch_occupant(wt, branch)
    own = wt.resolve()
    if occupant is not None and occupant.resolve() != own:
        sibling = occupant.parent.resolve() == own.parent and worker_slots.SLOT_NAME.match(occupant.name)
        if sibling and not _occupant_is_replaceable(occupant, own.parent):
            return EXIT_BRANCH_BUSY, f"BRANCH BUSY {occupant}"
        if sibling and not _is_dirty(occupant):
            git(occupant, "switch", "--detach")
        else:
            # The user's checkout, any other worktree, or changes for the next
            # owner of that slot to rescue: leave it alone and work detached.
            git(wt, "switch", "--detach", remote_ref)
            sha = git(wt, "rev-parse", "HEAD")
            return 0, (
                f"DETACHED {branch} (push with: git push "
                f"--force-with-lease=refs/heads/{branch}:{sha} origin HEAD:refs/heads/{branch})"
            )
    saved = _save_unpushed_commits(wt, branch, remote_ref)
    git(wt, "switch", "-C", branch, remote_ref)
    return 0, f"SWITCHED {branch} (unpushed commits saved at {saved})" if saved else f"SWITCHED {branch}"


def _branch_exists(wt: Path, ref: str) -> bool:
    try:
        git(wt, "rev-parse", "--verify", "--quiet", ref)
    except WorktreeError:
        return False
    return True


def _commit_identity(wt: Path) -> list[str]:
    """The user's identity when configured, else a recognisable fallback."""
    try:
        git(wt, "config", "user.email")
        return []
    except WorktreeError:
        return ["-c", "user.name=worker-loop", "-c", "user.email=worker-loop@localhost"]


def _commit_wip(dead_wt: Path, dead: str) -> str | None:
    """Commit everything uncommitted in the dead worktree. Returns the commit."""
    _abort_in_progress(dead_wt)
    if not _is_dirty(dead_wt):
        return None
    git(dead_wt, "add", "--all")
    # A snapshot of unfinished work must not be lost to hooks that check finished work.
    git(dead_wt, *_commit_identity(dead_wt), "commit", "--quiet", "--no-verify",
        "-m", f"WIP: worker-loop takeover of {dead}")
    return git(dead_wt, "rev-parse", "HEAD")


def _set_current(worker: str, current: dict | None) -> None:
    path = worker_state.state_path(worker)
    state = worker_state.read_json(path)
    state["current"] = current
    worker_state.atomic_write_json(path, state)


def takeover(wt: Path, worker: str, token: str, dead: str, threshold: int | None) -> tuple[int, str]:
    """Resume the `current` work of a slot whose run died without unlocking."""
    if dead == worker or not worker_slots.SLOT_NAME.match(dead):
        return EXIT_ERROR, f"ERROR: {dead!r} is not another worker slot"
    if worker_state.read_json(worker_state.state_path(worker)).get("current"):
        return EXIT_ERROR, f"ERROR: {worker} already has current work; finish it before taking over"
    if _is_dirty(wt):
        return EXIT_ERROR, f"ERROR: {wt} has uncommitted changes; reset it first"
    orphan = next((o for o in worker_slots.orphaned_slots(threshold) if o["worker"] == dead), None)
    if orphan is None:
        return EXIT_BRANCH_BUSY, f"NOT ORPHANED {dead}: its run is live, recent, or left no current work"
    ref, branch = orphan["ref"], orphan.get("branch")
    holder = worker_claims.claimed_by_other(worker_claims.load_claims(), ref, dead)
    if holder and holder != worker:
        return EXIT_BRANCH_BUSY, f"CLAIMED {ref} by {holder}"
    dead_wt = wt.parent / dead
    if branch:
        if not (_branch_exists(wt, f"refs/heads/{branch}") or _branch_exists(wt, f"refs/remotes/origin/{branch}")):
            return EXIT_ERROR, f"ERROR: branch {branch} of {ref} exists neither locally nor on origin"
        occupant = _branch_occupant(wt, branch)
        if occupant is not None and occupant.resolve() not in (wt.resolve(), dead_wt.resolve()):
            return EXIT_BRANCH_BUSY, f"BRANCH BUSY {occupant}"

    # From here the dead run is fenced: its token can no longer renew the slot,
    # and no new tick can acquire the slot until end_handoff.
    fenced = worker_slots.fence_orphan(dead, worker, token, threshold)
    if fenced is None:
        return EXIT_BRANCH_BUSY, f"NOT ORPHANED {dead}: it changed while taking over"
    handed_over = False
    wip = None
    try:
        if branch:
            # A rebase or merge in progress detaches HEAD, which hides the branch
            # from `git worktree list`; aborting it checks the branch out again.
            if (dead_wt / ".git").exists():
                _abort_in_progress(dead_wt)
            occupant = _branch_occupant(wt, branch)
            if occupant is not None and occupant.resolve() == dead_wt.resolve():
                wip = _commit_wip(dead_wt, dead)
                git(dead_wt, "switch", "--detach")
            if _branch_exists(wt, f"refs/heads/{branch}"):
                git(wt, "switch", branch)
            else:
                git(wt, "switch", "-c", branch, f"origin/{branch}")
        holder = worker_claims.transfer(ref, dead, worker)
        if holder:
            return EXIT_BRANCH_BUSY, f"CLAIMED {ref} by {holder} (slot {dead} stays orphaned)"
        _set_current(worker, {"ref": ref, "branch": branch})
        _set_current(dead, None)
        handed_over = True
    finally:
        worker_slots.end_handoff(dead, fenced, handed_over)
    renewed = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(orphan["lastRenewedAt"]))
    line = f"TOOK OVER {ref} from {dead} on {branch or '(no branch)'} (last renewed {renewed})"
    return 0, f"{line}; uncommitted changes committed as {wip}" if wip else line


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=["reset", "switch", "takeover"])
    parser.add_argument("--worker", required=True)
    parser.add_argument("--token", required=True)
    parser.add_argument("--wt", type=Path, required=True)
    parser.add_argument("--branch", default=None)
    parser.add_argument("--from", dest="dead", default=None, help="takeover: the orphaned slot")
    parser.add_argument("--orphan-after", type=int, default=None,
                        help=f"takeover: seconds of silence (default ${worker_slots.ORPHAN_AFTER_ENV} or "
                             f"{worker_slots.DEFAULT_ORPHAN_AFTER_SECONDS})")
    args = parser.parse_args(argv)
    if args.command == "switch" and not args.branch:
        parser.error("switch requires --branch")
    if args.command == "takeover" and not args.dead:
        parser.error("takeover requires --from")
    if args.orphan_after is not None and args.orphan_after <= 0:
        parser.error("--orphan-after must be positive")

    try:
        wt = args.wt.resolve()
        if wt.name != args.worker:
            print(f"ERROR: {wt} is not the worktree of slot {args.worker}")
            return EXIT_ERROR
        if not lease.renew(args.worker, args.token):
            print(f"LEASE LOST: {args.worker} is not held by this token; nothing changed")
            return EXIT_LEASE_LOST
        if args.command == "reset":
            print(reset(wt, args.worker))
            return 0
        if args.command == "takeover":
            code, line = takeover(wt, args.worker, args.token, args.dead, args.orphan_after)
        else:
            code, line = switch(wt, args.branch)
        print(line)
        return code
    except (WorktreeError, worker_state.StateFileCorrupt, TimeoutError, OSError, ValueError) as e:
        print(f"ERROR: {e}")
        return EXIT_ERROR


if __name__ == "__main__":
    sys.exit(main())
