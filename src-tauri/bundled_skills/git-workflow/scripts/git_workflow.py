#!/usr/bin/env python3
"""
LibrAgent git + GitHub CLI helpers for end-user repository workflows.

Uses local `git` and `gh` (GitHub CLI). JSON on stdout; errors on stderr.

Actions:
  check_prereqs    — verify git repo, gh auth, clean enough state
  repo_context     — remote, default branch, current branch
  create_branch    — create and checkout branch
  status           — porcelain status summary
  commit           — commit staged or all tracked changes
  push             — push current branch (set upstream)
  pr_create        — gh pr create
  pr_view          — gh pr view JSON
  pr_checks        — CI/check status for a PR
  pr_merge         — merge PR via gh
  log_since_tag    — commits since last tag (release notes helper)
  release_create   — gh release create
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any


def error_exit(message: str, code: int = 1) -> None:
    print(json.dumps({"error": message}), file=sys.stderr)
    sys.exit(code)


def run(cmd: list[str], *, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=cwd,
        text=True,
        capture_output=True,
        check=False,
    )


def require_ok(result: subprocess.CompletedProcess[str], action: str) -> str:
    if result.returncode != 0:
        detail = (result.stderr or result.stdout or "").strip()
        error_exit(f"{action} failed: {detail}")
    return (result.stdout or "").strip()


def repo_root() -> Path:
    result = run(["git", "rev-parse", "--show-toplevel"])
    if result.returncode != 0:
        error_exit("Not inside a git repository.")
    return Path(require_ok(result, "git rev-parse"))


def action_check_prereqs(_: argparse.Namespace) -> dict[str, Any]:
    root = repo_root()
    git_version = require_ok(run(["git", "--version"], cwd=root), "git --version")
    gh_result = run(["gh", "auth", "status"], cwd=root)
    gh_ok = gh_result.returncode == 0
    payload: dict[str, Any] = {
        "action": "check_prereqs",
        "repo_root": str(root),
        "git": git_version,
        "gh_authenticated": gh_ok,
    }
    if not gh_ok:
        payload["gh_hint"] = "Run: gh auth login"
        payload["warning"] = "GitHub PR/merge/release actions need gh authentication."
    else:
        payload["gh_status"] = (gh_result.stdout or gh_result.stderr or "").strip()
    return payload


def action_repo_context(_: argparse.Namespace) -> dict[str, Any]:
    root = repo_root()
    branch = require_ok(run(["git", "branch", "--show-current"], cwd=root), "current branch")
    remote = run(["git", "remote", "get-url", "origin"], cwd=root)
    remote_url = (remote.stdout or "").strip() if remote.returncode == 0 else None
    default_branch = "main"
    default = run(["gh", "repo", "view", "--json", "defaultBranchRef"], cwd=root)
    if default.returncode == 0 and default.stdout:
        try:
            default_branch = json.loads(default.stdout)["defaultBranchRef"]["name"]
        except (json.JSONDecodeError, KeyError, TypeError):
            pass
    else:
        sym = run(["git", "symbolic-ref", "refs/remotes/origin/HEAD"], cwd=root)
        if sym.returncode == 0 and sym.stdout:
            default_branch = sym.stdout.strip().replace("refs/remotes/origin/", "")
    return {
        "action": "repo_context",
        "repo_root": str(root),
        "current_branch": branch,
        "default_branch": default_branch,
        "origin": remote_url,
    }


def action_create_branch(args: argparse.Namespace) -> dict[str, Any]:
    root = repo_root()
    name = args.name.strip()
    if not re.fullmatch(r"[A-Za-z0-9._/-]+", name):
        error_exit("Branch name contains invalid characters.")
    base = args.base
    if base:
        require_ok(run(["git", "fetch", "origin", base], cwd=root), "git fetch")
        require_ok(run(["git", "checkout", "-B", name, f"origin/{base}"], cwd=root), "checkout base")
    else:
        require_ok(run(["git", "checkout", "-b", name], cwd=root), "git checkout -b")
    return {"action": "create_branch", "branch": name, "base": base}


def action_status(_: argparse.Namespace) -> dict[str, Any]:
    root = repo_root()
    branch = require_ok(run(["git", "branch", "--show-current"], cwd=root), "branch")
    porcelain = require_ok(run(["git", "status", "--porcelain"], cwd=root), "status")
    lines = [line for line in porcelain.splitlines() if line.strip()]
    return {
        "action": "status",
        "branch": branch,
        "dirty": bool(lines),
        "changes": lines,
        "change_count": len(lines),
    }


def action_commit(args: argparse.Namespace) -> dict[str, Any]:
    root = repo_root()
    if args.all:
        require_ok(run(["git", "add", "-A"], cwd=root), "git add")
    sha = require_ok(
        run(["git", "commit", "-m", args.message], cwd=root),
        "git commit",
    )
    head = require_ok(run(["git", "rev-parse", "HEAD"], cwd=root), "rev-parse")
    return {"action": "commit", "message": args.message, "output": sha, "sha": head}


def action_push(args: argparse.Namespace) -> dict[str, Any]:
    root = repo_root()
    branch = require_ok(run(["git", "branch", "--show-current"], cwd=root), "branch")
    cmd = ["git", "push", "-u", "origin", branch] if args.set_upstream else ["git", "push", "origin", branch]
    out = require_ok(run(cmd, cwd=root), "git push")
    return {"action": "push", "branch": branch, "output": out}


def action_pr_create(args: argparse.Namespace) -> dict[str, Any]:
    root = repo_root()
    cmd = ["gh", "pr", "create", "--title", args.title, "--body", args.body]
    if args.base:
        cmd.extend(["--base", args.base])
    if args.draft:
        cmd.append("--draft")
    out = require_ok(run(cmd, cwd=root), "gh pr create")
    number = None
    match = re.search(r"/pull/(\d+)", out)
    if match:
        number = int(match.group(1))
    return {"action": "pr_create", "url": out, "number": number, "draft": args.draft}


def action_pr_view(args: argparse.Namespace) -> dict[str, Any]:
    root = repo_root()
    cmd = ["gh", "pr", "view", str(args.number), "--json", "number,title,state,url,headRefName,baseRefName,mergeable,statusCheckRollup"]
    out = require_ok(run(cmd, cwd=root), "gh pr view")
    return {"action": "pr_view", "pr": json.loads(out)}


def action_pr_checks(args: argparse.Namespace) -> dict[str, Any]:
    root = repo_root()
    cmd = ["gh", "pr", "checks", str(args.number), "--json", "name,state,link,workflow"]
    result = run(cmd, cwd=root)
    if result.returncode != 0:
        # Older gh versions may not support --json on checks
        text = require_ok(
            run(["gh", "pr", "checks", str(args.number)], cwd=root),
            "gh pr checks",
        )
        return {"action": "pr_checks", "number": args.number, "raw": text}
    return {"action": "pr_checks", "number": args.number, "checks": json.loads(result.stdout or "[]")}


def action_pr_merge(args: argparse.Namespace) -> dict[str, Any]:
    root = repo_root()
    cmd = ["gh", "pr", "merge", str(args.number), f"--{args.method}"]
    if args.delete_branch:
        cmd.append("--delete-branch")
    out = require_ok(run(cmd, cwd=root), "gh pr merge")
    return {"action": "pr_merge", "number": args.number, "method": args.method, "output": out}


def action_log_since_tag(args: argparse.Namespace) -> dict[str, Any]:
    root = repo_root()
    tag_result = run(["git", "describe", "--tags", "--abbrev=0"], cwd=root)
    if tag_result.returncode != 0:
        range_ref = args.since or "HEAD~20"
        tag = None
    else:
        tag = (tag_result.stdout or "").strip()
        range_ref = f"{tag}..HEAD"
    log = require_ok(
        run(["git", "log", range_ref, "--no-merges", "--pretty=format:%h %s"], cwd=root),
        "git log",
    )
    commits = [{"raw": line} for line in log.splitlines() if line.strip()]
    return {
        "action": "log_since_tag",
        "since_tag": tag,
        "range": range_ref,
        "commits": commits,
        "count": len(commits),
    }


def action_release_create(args: argparse.Namespace) -> dict[str, Any]:
    root = repo_root()
    cmd = ["gh", "release", "create", args.tag, "--title", args.title]
    if args.notes_file:
        cmd.extend(["--notes-file", args.notes_file])
    elif args.notes:
        cmd.extend(["--notes", args.notes])
    else:
        cmd.append("--generate-notes")
    if args.draft:
        cmd.append("--draft")
    if args.prerelease:
        cmd.append("--prerelease")
    out = require_ok(run(cmd, cwd=root), "gh release create")
    return {"action": "release_create", "tag": args.tag, "url": out}


ACTIONS = {
    "check_prereqs": action_check_prereqs,
    "repo_context": action_repo_context,
    "create_branch": action_create_branch,
    "status": action_status,
    "commit": action_commit,
    "push": action_push,
    "pr_create": action_pr_create,
    "pr_view": action_pr_view,
    "pr_checks": action_pr_checks,
    "pr_merge": action_pr_merge,
    "log_since_tag": action_log_since_tag,
    "release_create": action_release_create,
}


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="LibrAgent git workflow CLI")
    parser.add_argument("--action", required=True, choices=sorted(ACTIONS))
    parser.add_argument("--name", help="Branch name")
    parser.add_argument("--base", help="Base branch for branch/PR")
    parser.add_argument("--message", help="Commit message")
    parser.add_argument("--all", action="store_true", help="Stage all before commit")
    parser.add_argument("--set-upstream", action="store_true", default=True)
    parser.add_argument("--title")
    parser.add_argument("--body", default="")
    parser.add_argument("--draft", action="store_true")
    parser.add_argument("--number", type=int)
    parser.add_argument("--method", choices=["merge", "squash", "rebase"], default="squash")
    parser.add_argument("--delete-branch", action="store_true")
    parser.add_argument("--tag")
    parser.add_argument("--notes")
    parser.add_argument("--notes-file")
    parser.add_argument("--prerelease", action="store_true")
    parser.add_argument("--since")
    return parser


def validate_args(args: argparse.Namespace) -> None:
    if args.action == "create_branch" and not args.name:
        raise ValueError("--name is required for create_branch")
    if args.action == "commit" and not args.message:
        raise ValueError("--message is required for commit")
    if args.action == "pr_create" and not args.title:
        raise ValueError("--title is required for pr_create")
    if args.action in {"pr_view", "pr_checks", "pr_merge"} and not args.number:
        raise ValueError("--number is required for PR actions")
    if args.action == "release_create" and not args.tag:
        raise ValueError("--tag is required for release_create")
    if args.action == "release_create" and not args.title:
        raise ValueError("--title is required for release_create")


def main() -> int:
    args = build_parser().parse_args()
    try:
        validate_args(args)
        payload = ACTIONS[args.action](args)
    except SystemExit:
        raise
    except Exception as exc:
        error_exit(str(exc))
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
