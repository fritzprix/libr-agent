#!/usr/bin/env python3
"""Heuristic map from git diff paths to #2565 manifest case ids (hints only)."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

# path prefix -> manifest ids (suggestions, not authoritative)
_PREFIX_HINTS: list[tuple[str, list[str]]] = [
    ("src-tauri/src/mcp/builtin/workspace", ["B-3", "B-4", "B-5", "B-6", "B-11", "O-10"]),
    ("src-tauri/src/server/e2e_control", ["B-3", "P-7"]),
    ("benchmarks/harbor/libragent-session-e2e-matrix", ["O-10", "B-3", "B-6", "B-11"]),
    ("src-tauri/src/mcp/builtin/mcp", ["M-1", "M-2", "M-3", "M-5"]),
    ("src/features/", ["N-1", "N-2", "N-3", "N-4", "N-5", "A-1", "A-2", "A-3"]),
    ("src-tauri/src/agent/pending", ["P-1", "P-2", "P-3", "P-4", "P-5", "P-6", "P-7"]),
]


def _changed_files(repo: Path, baseline: str, head: str = "HEAD") -> list[str]:
    proc = subprocess.run(
        [
            "git",
            "-C",
            str(repo),
            "diff",
            "--name-only",
            f"{baseline}..{head}",
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.strip() or "git diff failed")
    return [line for line in proc.stdout.splitlines() if line.strip()]


def map_diff_to_cases(
    repo: Path,
    baseline: str,
    manifest: Path,
    *,
    head: str = "HEAD",
) -> dict:
    files = _changed_files(repo, baseline, head)
    manifest_data = json.loads(manifest.read_text(encoding="utf-8"))
    known_ids = {c["id"] for c in manifest_data.get("cases", [])}
    hinted: dict[str, list[str]] = {}
    for path in files:
        for prefix, ids in _PREFIX_HINTS:
            if path.startswith(prefix) or prefix in path:
                for cid in ids:
                    if cid in known_ids:
                        hinted.setdefault(cid, []).append(path)
    commits = subprocess.run(
        [
            "git",
            "-C",
            str(repo),
            "log",
            f"{baseline}..{head}",
            "--no-merges",
            "--oneline",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    return {
        "baseline": baseline,
        "head": head,
        "changedFiles": files,
        "commitOneline": [
            line for line in commits.stdout.splitlines() if line.strip()
        ],
        "caseHints": [
            {"id": cid, "becauseFiles": paths}
            for cid, paths in sorted(hinted.items())
        ],
        "note": "Hints only — agent must tie each case to spec/diff manually.",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, default=Path("."))
    parser.add_argument("--baseline", required=True, help="tag or sha")
    parser.add_argument("--head", default="HEAD")
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    data = map_diff_to_cases(
        args.repo.resolve(), args.baseline, args.manifest.resolve(), head=args.head
    )
    if args.json:
        print(json.dumps(data, indent=2))
    else:
        print(data)
    return 0


if __name__ == "__main__":
    sys.exit(main())
