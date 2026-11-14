#!/usr/bin/env python3
"""Report which PR merge commits are contained in a running build or a given SHA.

Reads buildSha from GET /api/health or GET /api/e2e-control/status, or from --build-sha.
For each PR number, loads the merge commit and runs git merge-base --is-ancestor.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path


class GhError(RuntimeError):
    pass


class ContainmentError(RuntimeError):
    pass


def gh(*args: str) -> str:
    result = subprocess.run(
        ["gh", *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if result.returncode != 0:
        raise GhError(f"gh {' '.join(args[:4])} failed: {result.stderr.strip()}")
    return result.stdout


def fetch_json(url: str, headers: dict[str, str] | None = None) -> dict:
    req = urllib.request.Request(url, headers=headers or {}, method="GET")
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.URLError as exc:
        raise ContainmentError(f"GET {url} failed: {exc}") from exc


def provenance_from_url(status_url: str, e2e_cap: str | None) -> tuple[str, bool]:
    headers: dict[str, str] = {}
    if e2e_cap:
        headers["X-LibrAgent-E2E-Control"] = e2e_cap
    data = fetch_json(status_url, headers)
    sha = data.get("buildSha")
    if not isinstance(sha, str) or not sha.strip():
        raise ContainmentError(f"response has no buildSha: {status_url}")
    dirty = bool(data.get("buildDirty"))
    return sha.strip(), dirty


def merge_commit(pr: int) -> str:
    out = gh("pr", "view", str(pr), "--json", "mergeCommit,state")
    payload = json.loads(out or "{}")
    if payload.get("state") != "MERGED":
        raise ContainmentError(f"PR #{pr} is not merged (state={payload.get('state')!r})")
    commit = (payload.get("mergeCommit") or {}).get("oid")
    if not isinstance(commit, str) or len(commit) < 7:
        raise ContainmentError(f"PR #{pr} has no merge commit")
    return commit


def is_ancestor(repo: Path, ancestor: str, descendant: str) -> bool:
    if ancestor == "unknown" or descendant == "unknown":
        return False
    result = subprocess.run(
        ["git", "-C", str(repo), "merge-base", "--is-ancestor", ancestor, descendant],
        capture_output=True,
    )
    return result.returncode == 0


@dataclass
class PrResult:
    pr: int
    merge_commit: str
    contained: bool


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prs", required=True, help="Comma-separated PR numbers")
    parser.add_argument("--repo", type=Path, default=Path.cwd(), help="Git repo for merge-base checks")
    parser.add_argument("--build-sha", default=None, help="Build SHA (skip HTTP)")
    parser.add_argument("--status-url", default=None, help="Health or e2e-control status URL")
    parser.add_argument("--e2e-cap", default=None, help="X-LibrAgent-E2E-Control header value")
    parser.add_argument("--json", action="store_true", help="Print machine-readable report")
    args = parser.parse_args(argv)

    prs = [int(n.strip()) for n in args.prs.split(",") if n.strip()]
    if not prs:
        sys.stderr.write("check_pr_build_containment: no PR numbers\n")
        return 2

    try:
        if args.build_sha:
            build_sha = args.build_sha.strip()
            build_dirty = False
        elif args.status_url:
            build_sha, build_dirty = provenance_from_url(args.status_url, args.e2e_cap)
        else:
            raise ContainmentError("pass --build-sha or --status-url")

        rows: list[PrResult] = []
        for pr in prs:
            merge = merge_commit(pr)
            rows.append(
                PrResult(
                    pr=pr,
                    merge_commit=merge,
                    contained=is_ancestor(args.repo.resolve(), merge, build_sha),
                )
            )
    except (GhError, ContainmentError, ValueError, OSError) as exc:
        sys.stderr.write(f"check_pr_build_containment: {exc}\n")
        return 3

    if args.json:
        print(
            json.dumps(
                {
                    "buildSha": build_sha,
                    "buildDirty": build_dirty,
                    "prs": [
                        {
                            "pr": r.pr,
                            "mergeCommit": r.merge_commit,
                            "contained": r.contained,
                            "verdict": "contained" if r.contained else "blocked: not in build",
                        }
                        for r in rows
                    ],
                },
                indent=2,
            )
        )
        missing = [r.pr for r in rows if not r.contained]
        return 1 if missing else 0

    print(f"buildSha={build_sha} buildDirty={build_dirty}")
    for row in rows:
        verdict = "contained" if row.contained else "blocked: not in build"
        print(f"PR #{row.pr} merge={row.merge_commit} {verdict}")
    missing = [r.pr for r in rows if not r.contained]
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
