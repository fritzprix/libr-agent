#!/usr/bin/env python3
"""Worktree operations a tick must not run unguarded.

  worker_worktree.py reset  --worker W --token T --wt PATH
  worker_worktree.py switch --worker W --token T --wt PATH --branch B

A slot's worktree belongs to whoever holds the slot, so each command first renews
the lease and refuses if it is lost or PATH is not this slot's worktree. `reset`
saves uncommitted changes in a stash before discarding anything, so changes left
by an earlier run stay recoverable. `switch` will not take a branch from a live
run, frees it from an abandoned, clean worktree, and keeps a ref to any local
commits that resetting the branch to the remote would drop.

Prints one result line. Exit codes:
  0 - done: READY | RESET | RESCUED <stash> | SWITCHED <branch> | DETACHED <branch>
  2 - BRANCH BUSY <path>: another run needs that branch; skip this item
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


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=["reset", "switch"])
    parser.add_argument("--worker", required=True)
    parser.add_argument("--token", required=True)
    parser.add_argument("--wt", type=Path, required=True)
    parser.add_argument("--branch", default=None)
    args = parser.parse_args(argv)
    if args.command == "switch" and not args.branch:
        parser.error("switch requires --branch")

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
        code, line = switch(wt, args.branch)
        print(line)
        return code
    except (WorktreeError, worker_state.StateFileCorrupt, TimeoutError, OSError) as e:
        print(f"ERROR: {e}")
        return EXIT_ERROR


if __name__ == "__main__":
    sys.exit(main())
