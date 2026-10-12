#!/usr/bin/env python3
"""Scan latest knowledge-bridge Harbor jobs for record/search tool sequences."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
JOBS = REPO / "jobs"

TOOL_RE = re.compile(
    r"\b(knowledge__(?:recordKnowledge|searchKnowledge|exploreContext|pruneKnowledge))\b"
)


def tools_in_traj(path: Path) -> list[str]:
    text = path.read_text(encoding="utf-8", errors="replace")
    found: list[str] = []
    try:
        data = json.loads(text)
    except json.JSONDecodeError:
        return TOOL_RE.findall(text)
    for step in data.get("steps") or []:
        if not isinstance(step, dict):
            continue
        agent = step.get("agent") or {}
        for call in agent.get("tool_calls") or []:
            if isinstance(call, dict) and isinstance(call.get("name"), str):
                name = call["name"]
                if name.startswith("knowledge__"):
                    found.append(name)
    if not found:
        found = TOOL_RE.findall(text)
    return found


def main() -> int:
    if not JOBS.is_dir():
        print("No jobs/ directory", file=sys.stderr)
        return 1

    # Prefer jobs that mention our task names
    candidates: list[Path] = []
    for job in sorted(JOBS.iterdir(), reverse=True):
        if not job.is_dir():
            continue
        name = job.name
        blob = " ".join(p.name for p in job.iterdir() if p.is_dir())
        if "kb-research" in blob or "kb-cold" in blob or "knowledge" in blob.lower():
            candidates.append(job)
        elif not candidates and name.startswith("2026-"):
            # keep scanning; don't add unrelated yet
            pass

    if not candidates:
        # fallback: two newest jobs
        candidates = sorted(
            [p for p in JOBS.iterdir() if p.is_dir() and p.name.startswith("20")],
            reverse=True,
        )[:2]

    ok_record = False
    ok_search = False
    reports: list[str] = []

    for job in candidates[:6]:
        trajs = list(job.rglob("trajectory.json"))
        for traj in trajs:
            tools = tools_in_traj(traj)
            rel = traj.relative_to(REPO)
            reports.append(f"{rel}: {tools}")
            if "knowledge__recordKnowledge" in tools:
                ok_record = True
            if "knowledge__searchKnowledge" in tools:
                ok_search = True

    print("Trajectory tool scan:")
    for line in reports:
        print(" ", line)

    if ok_record and ok_search:
        print("PASS: found knowledge__recordKnowledge and knowledge__searchKnowledge")
        return 0

    print(
        "FAIL: need both knowledge__recordKnowledge and knowledge__searchKnowledge "
        f"(record={ok_record}, search={ok_search})",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
