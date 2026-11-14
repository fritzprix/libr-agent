#!/usr/bin/env python3
"""Preflight gate for worker-loop.

Computes a fingerprint from a single GraphQL request covering:
  - Open PRs authored by @me: headRefOid, mergeable, CI statusCheckRollup state,
    updatedAt, and last comment createdAt.
  - Candidate issues: updatedAt timestamps.

With no 'current' work and an unchanged fingerprint, it still returns RUN when an
open `approved` issue is free to take and no open PR closes it, so an approved
issue is never ignored just because the tracked state did not move.

If state.json has no 'current' and nothing is pending and the fingerprint matches
'lastFingerprint', outputs 'NOOP' and exits with code 1 so external schedulers
(e.g., `python3 tick_gate.py && ...`) do not invoke the LLM.

Exit codes:
  0 - RUN (actionable work or state changed)
  1 - NOOP (state unchanged, idle, nothing pending)
  2 - BUSY (no slot for this tick; the printed reason says why, and only
      `slot-held` or `no-free-slot` means other workers exist)
  3 - ERROR (authentication, network, or GraphQL failure; not NOOP)
  4 - LEASE LOST (this tick no longer owns its slot: stop working, touch nothing)
  5 - SKILLS STALE (.agents/skills differs from origin/<base>; stop the tick)

--lock prints `WORKER <name>` and `TOKEN <token>`. Pass both to every later
command of the tick. --renew extends the lease and is the ownership check before
destructive git operations and pushes. Long commands run under lease_run.py.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path

import issue_candidates
import lease
import tick_lock
import worker_claims
import worker_slots
import worker_state

_LOOP_SHARED = Path(__file__).resolve().parents[2] / "loop-shared" / "scripts"
if str(_LOOP_SHARED) not in sys.path:
    sys.path.insert(0, str(_LOOP_SHARED))

import skills_freshness  # noqa: E402

EXIT_RUN = 0
EXIT_NOOP = 1
EXIT_BUSY = 2
EXIT_ERROR = 3
EXIT_LEASE_LOST = worker_state.EXIT_LEASE_LOST

# Without renewal a lease ends after 30 minutes; lease_run.py renews during builds.
LOCK_TTL_SECONDS = worker_state.LEASE_TTL_SECONDS

# 30 candidates covers active queue without query bloat; matches worker-loop Step 2 fallback limit.
ISSUE_LIMIT = 30
PR_LIMIT = 30

GRAPHQL_GATE_QUERY = """
query($prQuery: String!, $prLimit: Int!, $owner: String!, $name: String!, $issueLimit: Int!) {
  search(query: $prQuery, type: ISSUE, first: $prLimit) {
    nodes {
      ... on PullRequest {
        number
        headRefOid
        mergeable
        updatedAt
        isDraft
        author { login }
        commits(last: 1) {
          nodes {
            commit {
              statusCheckRollup {
                state
              }
            }
          }
        }
        comments(last: 1) {
          nodes {
            createdAt
          }
        }
      }
    }
  }
  repository(owner: $owner, name: $name) {
    issues(first: $issueLimit, states: [OPEN], orderBy: {field: UPDATED_AT, direction: DESC}) {
      nodes {
        number
        updatedAt
      }
    }
  }
}
"""


class GhError(RuntimeError):
    """Raised when GitHub CLI execution fails."""


def default_state_path(worker: str) -> Path:
    """Each worker has its own state file and lock, so workers never block each other."""
    return worker_state.worker_dir(worker) / "state.json"


def load_state(path: Path) -> dict:
    return worker_state.read_json(path)


def save_state(path: Path, state: dict) -> None:
    """Refuse to write over a state file we cannot read; that is the #2612 wipe."""
    if path.is_file():
        worker_state.read_json(path)
    worker_state.atomic_write_json(path, state)


def gh(*args: str) -> str:
    try:
        result = subprocess.run(["gh", *args], capture_output=True, text=True, encoding="utf-8")
    except Exception as e:
        raise GhError(f"Failed to execute gh: {e}") from e
    if result.returncode != 0:
        raise GhError(f"gh {' '.join(args)} failed (code {result.returncode}): {result.stderr.strip()}")
    return result.stdout


def check_lock(lock_file: Path) -> bool:
    """Return True if an active, non-stale lock exists."""
    return tick_lock.is_held(lock_file)


def acquire_lock(lock_file: Path, pid: int | None = None) -> bool:
    """Lock for single-instance loops (producer-loop); False while another run holds it."""
    return tick_lock.acquire(lock_file, None, pid=pid) is not None


def release_lock(lock_file: Path) -> None:
    tick_lock.release(lock_file, None)


def pending_approved_issue(worker: str) -> int | None:
    """First open `approved` issue this worker can take, or None.

    Uses `issue_candidates.exclusion_reason` so "no open PR closes it" means the
    same thing here as in candidate selection.
    """
    claims = worker_claims.load_claims()
    me = gh("api", "user", "--jq", ".login").strip()
    issues = json.loads(
        gh("issue", "list", "--state", "open", "--limit", str(ISSUE_LIMIT), "--json",
           "number,labels,assignees")
    )
    closing_pr = issue_candidates.closing_pr_map(issue_candidates.list_open_prs())

    for issue in sorted(issues, key=lambda i: i["number"]):
        number = issue["number"]
        labels = {label["name"] for label in issue["labels"]}
        assignees = [a["login"] for a in issue["assignees"]]
        free = issue_candidates.exclusion_reason(
            number, labels, assignees, me, closing_pr,
            claimed_by=worker_claims.claimed_by_other(claims, f"issue:{number}", worker),
        )
        if free is None:
            return number
    return None


def compute_fingerprint() -> str:
    """Compute sha256 hash using a single GraphQL query for PRs and issues."""
    repo_raw = gh("repo", "view", "--json", "owner,name")
    repo = json.loads(repo_raw)
    owner, name = repo["owner"]["login"], repo["name"]
    me = gh("api", "user", "--jq", ".login").strip()
    pr_query = f"repo:{owner}/{name} is:pr is:open author:@me"

    query_raw = gh(
        "api", "graphql",
        "-f", f"query={GRAPHQL_GATE_QUERY}",
        "-F", f"prQuery={pr_query}",
        "-F", f"owner={owner}",
        "-F", f"name={name}",
        "-F", f"prLimit={PR_LIMIT}",
        "-F", f"issueLimit={ISSUE_LIMIT}",
    )
    data = json.loads(query_raw).get("data", {})
    repo_data = data.get("repository", {})

    pr_nodes = data.get("search", {}).get("nodes") or []
    # Only PRs authored by @me and not draft
    prs_fingerprint = []
    for pr in pr_nodes:
        if not pr:
            continue
        if (pr.get("author") or {}).get("login") != me:
            continue
        if pr.get("isDraft"):
            continue

        commit_nodes = (pr.get("commits") or {}).get("nodes") or []
        last_commit = commit_nodes[0].get("commit") if commit_nodes else {}
        ci_state = ((last_commit.get("statusCheckRollup") or {}).get("state")) or ""

        comment_nodes = (pr.get("comments") or {}).get("nodes") or []
        last_comment_time = comment_nodes[0].get("createdAt") if comment_nodes else ""

        prs_fingerprint.append((
            pr["number"],
            pr.get("headRefOid"),
            pr.get("mergeable"),
            ci_state,
            pr.get("updatedAt"),
            last_comment_time,
        ))
    prs_fingerprint.sort()

    issue_nodes = repo_data.get("issues", {}).get("nodes") or []
    issues_fingerprint = sorted((i["number"], i.get("updatedAt")) for i in issue_nodes)

    hasher = hashlib.sha256()
    hasher.update(json.dumps({"prs": prs_fingerprint, "issues": issues_fingerprint}).encode("utf-8"))
    return hasher.hexdigest()


def _finish_tick(args: argparse.Namespace, token: str | None) -> int:
    """--update and/or --unlock. Only the run that owns the lease may do either."""
    worker = args.worker or os.environ.get("WORKER_ID") or (token and worker_slots.find_worker_by_token(token))
    worker = worker_state.worker_name(worker or None)
    state_path = args.state_file or default_state_path(worker)
    if not token or not lease.is_held_by(worker, token, args.state_file):
        print(f"LEASE LOST: {worker} is not held by this token; nothing changed")
        return EXIT_LEASE_LOST
    try:
        state = load_state(state_path)
        if args.update:
            tick_start_fp = state.pop("tickStartFingerprint", None)
            if tick_start_fp:
                state["lastFingerprint"] = tick_start_fp
                print(f"UPDATED: {tick_start_fp[:12]}")
            save_state(state_path, state)
        elif "tickStartFingerprint" in state:
            del state["tickStartFingerprint"]
            save_state(state_path, state)
        if args.unlock:
            worker_claims.release_finished(worker, state)
        return EXIT_RUN
    except GhError as e:
        sys.stderr.write(f"tick_gate GitHub CLI error: {e}\n")
        return EXIT_ERROR
    except (worker_state.StateFileCorrupt, TimeoutError, OSError) as e:
        sys.stderr.write(f"tick_gate state file error: {e}\n")
        return EXIT_ERROR
    finally:
        if args.unlock:
            if os.environ.get(lease.INHERITED_TOKEN_ENV) == token:
                print("LOCK KEPT (held by lease_run.py)")
            else:
                lease.release(worker, token, args.state_file)
                print("UNLOCKED")


def _gate(state: dict, state_path: Path, worker: str, locking: bool) -> tuple[int, str]:
    """Decide RUN or NOOP for a slot this tick may use."""
    if state.get("current"):
        return EXIT_RUN, "RUN (current work in progress)"

    fp = compute_fingerprint()
    last_fp = state.get("lastFingerprint")
    message = f"RUN (changes detected: {fp[:12]})"
    if last_fp and last_fp == fp:
        # Nothing tracked moved, but an approved issue may still be waiting.
        pending = pending_approved_issue(worker)
        if pending is None:
            return EXIT_NOOP, "NOOP (no changes)"
        message = f"RUN (approved issue pending: #{pending})"

    # Keep the start-of-tick fingerprint; lastFingerprint is saved only by a
    # successful idle tick (--update).
    if locking:
        state["tickStartFingerprint"] = fp
        save_state(state_path, state)
    return EXIT_RUN, message


def _lock_and_gate(args: argparse.Namespace) -> int:
    stale = skills_freshness.gate_skills_freshness()
    if stale is not None:
        return stale

    requested = args.worker or os.environ.get("WORKER_ID")
    held = lease.inherited_lease(requested, args.state_file)
    if held is None:
        owner = worker_state.resolve_owner(args.pid, args.session)
        try:
            held = lease.acquire(owner, requested, args.state_file)
        except worker_slots.SlotBusy as e:
            print(f"BUSY reason={e.reason} ({e})")
            return EXIT_BUSY
    print(f"WORKER {held.worker}")
    print(f"TOKEN {held.token}")

    code = EXIT_ERROR
    try:
        state_path = args.state_file or default_state_path(held.worker)
        code, message = _gate(load_state(state_path), state_path, held.worker, locking=True)
        print(message)
        return code
    finally:
        # RUN hands the lease to the tick. Every other exit, including an
        # exception on its way out, must not leave the slot occupied.
        if code != EXIT_RUN and not held.inherited:
            lease.release(held.worker, held.token, args.state_file)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--worker",
        default=None,
        help="Worker slot (default: $WORKER_ID, else assigned on --lock)",
    )
    parser.add_argument("--state-file", type=Path, default=None, help="Path to state.json")
    parser.add_argument("--update", action="store_true", help="Update lastFingerprint in state.json after a successful tick")
    parser.add_argument("--lock", action="store_true", help="Take a slot and tick lock; prints WORKER and TOKEN")
    parser.add_argument("--unlock", action="store_true", help="End the tick: release its claims, lock, and slot")
    parser.add_argument("--renew", action="store_true", help="Extend the lease; exit 4 if this tick lost it")
    parser.add_argument("--force-unlock", action="store_true", help="Operator recovery: free --worker without proving ownership")
    parser.add_argument("--token", default=None, help="Tick token printed by --lock (default: $WORKER_LOOP_TOKEN)")
    parser.add_argument("--session", default=None, help="Stable session ID (default: $WORKER_LOOP_SESSION_ID, or detected)")
    parser.add_argument(
        "--pid",
        type=int,
        default=None,
        help="PID that lives as long as the agent session; auto-detected if omitted",
    )
    args = parser.parse_args()
    token = args.token or os.environ.get("WORKER_LOOP_TOKEN") or os.environ.get(lease.INHERITED_TOKEN_ENV)

    try:
        if args.force_unlock:
            if not args.worker:
                sys.exit("--force-unlock requires --worker")
            release_lock(lease.lock_path(args.worker, args.state_file))
            worker_slots.force_release_slot(args.worker)
            print(f"FORCE UNLOCKED {args.worker}")
            sys.exit(EXIT_RUN)

        if args.renew:
            worker = args.worker or os.environ.get("WORKER_ID") or (token and worker_slots.find_worker_by_token(token))
            if worker and token and lease.renew(worker, token, args.state_file):
                print(f"RENEWED {worker}")
                sys.exit(EXIT_RUN)
            print(f"LEASE LOST: {worker or 'this tick'} is not held by this token; stop working")
            sys.exit(EXIT_LEASE_LOST)

        if args.update or args.unlock:
            sys.exit(_finish_tick(args, token))

        if args.lock:
            sys.exit(_lock_and_gate(args))

        # Plain check: report without taking anything.
        worker = worker_state.worker_name(args.worker)
        if tick_lock.is_held(lease.lock_path(worker, args.state_file)):
            print(f"BUSY reason=lock-held (a tick lock is live in {worker})")
            sys.exit(EXIT_BUSY)
        state_path = args.state_file or default_state_path(worker)
        code, message = _gate(load_state(state_path), state_path, worker, locking=False)
        print(message)
        sys.exit(code)

    except GhError as e:
        sys.stderr.write(f"tick_gate GitHub CLI error: {e}\n")
        sys.exit(EXIT_ERROR)
    except worker_state.StateFileCorrupt as e:
        # Never write over a state file we failed to read (#2612).
        sys.stderr.write(f"tick_gate state file error: {e}\n")
        sys.exit(EXIT_ERROR)
    except Exception as e:
        sys.stderr.write(f"tick_gate unexpected error: {e}\n")
        sys.exit(EXIT_ERROR)


if __name__ == "__main__":
    main()
