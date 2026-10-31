#!/usr/bin/env python3
"""Preflight gate for worker-loop.

Computes a fingerprint from a single GraphQL request covering:
  - Open PRs authored by @me: headRefOid, mergeable, CI statusCheckRollup state,
    updatedAt, and last comment createdAt.
  - Candidate issues: updatedAt timestamps.

If state.json has no 'current' and the fingerprint matches 'lastFingerprint', outputs
'NOOP' and exits with code 1 so external schedulers (e.g., `python3 tick_gate.py && ...`)
do not invoke the LLM.

Exit codes:
  0 - RUN (actionable work or state changed)
  1 - NOOP (state unchanged and idle)
  2 - BUSY (another tick is in progress with an active lock)
  3 - ERROR (authentication, network, or GraphQL failure; not NOOP)
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

EXIT_RUN = 0
EXIT_NOOP = 1
EXIT_BUSY = 2
EXIT_ERROR = 3

# Lock TTL is 15 minutes, per issue #2418 (C3).
LOCK_TTL_SECONDS = 900

# 30 candidates covers active queue without query bloat; matches worker-loop Step 2 fallback limit.
ISSUE_LIMIT = 30
PR_LIMIT = 30

GRAPHQL_GATE_QUERY = """
query($owner: String!, $name: String!, $prLimit: Int!, $issueLimit: Int!) {
  repository(owner: $owner, name: $name) {
    pullRequests(first: $prLimit, states: [OPEN]) {
      nodes {
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


def default_state_path() -> Path:
    env = os.environ.get("LOOP_STATE_DIR")
    if env:
        p = Path(env)
        return (
            p
            if p.name == "state.json"
            else (p if p.name == "worker-loop" else p / "worker-loop") / "state.json"
        )
    try:
        out = subprocess.run(
            ["git", "rev-parse", "--git-common-dir"],
            capture_output=True, text=True, check=True
        ).stdout.strip()
        common = Path(out).resolve()
        repo_root = common.parent if common.name == ".git" else common
        return repo_root / ".libragent" / "work" / "worker-loop" / "state.json"
    except Exception:
        return Path(".libragent/work/worker-loop/state.json")


def load_state(path: Path) -> dict:
    if path.is_file():
        try:
            return json.loads(path.read_text(encoding="utf-8"))
        except Exception:
            return {}
    return {}


def save_state(path: Path, state: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(state, ensure_ascii=False, indent=2), encoding="utf-8")


def gh(*args: str) -> str:
    try:
        result = subprocess.run(["gh", *args], capture_output=True, text=True, encoding="utf-8")
    except Exception as e:
        raise GhError(f"Failed to execute gh: {e}") from e
    if result.returncode != 0:
        raise GhError(f"gh {' '.join(args)} failed (code {result.returncode}): {result.stderr.strip()}")
    return result.stdout


def check_lock(lock_file: Path, ttl_seconds: int = LOCK_TTL_SECONDS) -> bool:
    """Return True if an active, non-stale lock exists."""
    if not lock_file.is_file():
        return False
    try:
        data = json.loads(lock_file.read_text(encoding="utf-8"))
        locked_at = data.get("time", 0)
        now = time.time()
        # If lock expired by TTL, it is stale regardless of PID
        if now - locked_at > ttl_seconds:
            return False
        # If PID was explicitly recorded, verify if process is still alive
        pid = data.get("pid")
        if pid and sys.platform != "win32":
            try:
                os.kill(pid, 0)
            except OSError:
                # Process is dead, lock is stale
                return False
        return True
    except Exception:
        return False


def acquire_lock(lock_file: Path, pid: int | None = None) -> None:
    lock_file.parent.mkdir(parents=True, exist_ok=True)
    payload = {"time": int(time.time())}
    if pid is not None:
        payload["pid"] = pid
    lock_file.write_text(json.dumps(payload), encoding="utf-8")


def release_lock(lock_file: Path) -> None:
    if lock_file.is_file():
        try:
            lock_file.unlink()
        except OSError:
            pass


def compute_fingerprint() -> str:
    """Compute sha256 hash using a single GraphQL query for PRs and issues."""
    repo_raw = gh("repo", "view", "--json", "owner,name")
    repo = json.loads(repo_raw)
    owner, name = repo["owner"]["login"], repo["name"]
    me = gh("api", "user", "--jq", ".login").strip()

    query_raw = gh(
        "api", "graphql",
        "-f", f"query={GRAPHQL_GATE_QUERY}",
        "-F", f"owner={owner}",
        "-F", f"name={name}",
        "-F", f"prLimit={PR_LIMIT}",
        "-F", f"issueLimit={ISSUE_LIMIT}",
    )
    data = json.loads(query_raw).get("data", {}).get("repository", {})

    pr_nodes = data.get("pullRequests", {}).get("nodes") or []
    # Only PRs authored by @me and not draft
    prs_fingerprint = []
    for pr in prnodes if (prnodes := pr_nodes) else []:
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

    issue_nodes = data.get("issues", {}).get("nodes") or []
    issues_fingerprint = sorted((i["number"], i.get("updatedAt")) for i in issue_nodes)

    hasher = hashlib.sha256()
    hasher.update(json.dumps({"prs": prs_fingerprint, "issues": issues_fingerprint}).encode("utf-8"))
    return hasher.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--state-file", type=Path, default=None, help="Path to state.json")
    parser.add_argument("--update", action="store_true", help="Update lastFingerprint in state.json after a successful tick")
    parser.add_argument("--lock", action="store_true", help="Acquire tick.lock before exiting 0")
    parser.add_argument("--pid", type=int, default=None, help="Process ID of the tick owner to store in tick.lock")
    parser.add_argument("--unlock", action="store_true", help="Release tick.lock")
    args = parser.parse_args()

    state_path = args.state_file or default_state_path()
    lock_path = state_path.parent / "tick.lock"

    if args.unlock:
        release_lock(lock_path)
        print("UNLOCKED")
        sys.exit(EXIT_RUN)

    if check_lock(lock_path):
        print("BUSY (tick in progress)")
        sys.exit(EXIT_BUSY)

    state = load_state(state_path)

    try:
        # If currently implementing an issue or branch, always run
        if state.get("current"):
            if args.lock:
                acquire_lock(lock_path, pid=args.pid)
            print("RUN (current work in progress)")
            sys.exit(EXIT_RUN)

        if args.update:
            fp = compute_fingerprint()
            state["lastFingerprint"] = fp
            save_state(state_path, state)
            print(f"UPDATED: {fp[:12]}")
            sys.exit(EXIT_RUN)

        fp = compute_fingerprint()
        last_fp = state.get("lastFingerprint")

        if last_fp and last_fp == fp:
            print("NOOP (no changes)")
            sys.exit(EXIT_NOOP)

        # State changed: acquire lock if requested, but DO NOT save lastFingerprint yet.
        # lastFingerprint must be saved only after a successful tick via --update.
        if args.lock:
            acquire_lock(lock_path, pid=args.pid)
        print(f"RUN (changes detected: {fp[:12]})")
        sys.exit(EXIT_RUN)

    except GhError as e:
        sys.stderr.write(f"tick_gate GitHub CLI error: {e}\n")
        sys.exit(EXIT_ERROR)
    except Exception as e:
        sys.stderr.write(f"tick_gate unexpected error: {e}\n")
        sys.exit(EXIT_ERROR)


if __name__ == "__main__":
    main()
