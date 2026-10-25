#!/usr/bin/env python3
"""Grade layman-intent Harbor trials by each skill's critical path (#2183).

A trial passes when its trajectory contains every critical-path tool call of
the task's skill and the last such call succeeded. N tasks pass when they made
no forbidden calls. The rules live in
``benchmarks/harbor/layman_intent_critical_path.py``, shared with the
container verifiers.

Trials are matched to tasks by the ``task_name`` Harbor records in each
trial's ``result.json``.

Usage:
  scripts/check-layman-intent-trajectories.py [--jobs-dir jobs] [--job <name>]
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO / "benchmarks" / "harbor"))

from layman_intent_critical_path import TASKS, TaskSpec, grade_trajectory

SPECS_BY_NAME: dict[str, TaskSpec] = {
    name: spec for spec in TASKS.values() for name in (spec.harbor_name, spec.directory)
}


@dataclass
class Grade:
    spec: TaskSpec
    failures: list[str]

    @property
    def passed(self) -> bool:
        return not self.failures

    @property
    def label(self) -> str:
        return self.spec.skill or f"({self.spec.category})"


def find_trials(job_dir: Path) -> list[tuple[TaskSpec, Path]]:
    """Trials whose recorded task name belongs to this suite."""
    trials: list[tuple[TaskSpec, Path]] = []
    for result_path in sorted(job_dir.glob("*/result.json")):
        try:
            result = json.loads(result_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        task_name = result.get("task_name") if isinstance(result, dict) else None
        spec = SPECS_BY_NAME.get(task_name) if isinstance(task_name, str) else None
        if spec is not None:
            trials.append((spec, result_path.parent / "agent" / "trajectory.json"))
    return trials


def latest_job(jobs_dir: Path) -> Path | None:
    jobs = sorted(
        (path for path in jobs_dir.iterdir() if path.is_dir()),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    return next((job for job in jobs if find_trials(job)), None)


def render_report(grades: list[Grade]) -> str:
    lines = ["Per-task:"]
    for result in sorted(grades, key=lambda g: g.spec.task_id):
        status = "PASS" if result.passed else "FAIL"
        detail = "" if result.passed else " — " + "; ".join(result.failures)
        lines.append(f"  {result.spec.task_id} {result.label:<22} {status}{detail}")

    by_skill: dict[str, list[bool]] = defaultdict(list)
    for result in grades:
        by_skill[result.label].append(result.passed)
    lines.append("")
    lines.append("Per-skill:")
    for skill in sorted(by_skill):
        outcomes = by_skill[skill]
        lines.append(f"  {skill:<22} {sum(outcomes)}/{len(outcomes)}")

    passed = sum(result.passed for result in grades)
    lines.append("")
    lines.append(f"Routing accuracy: {passed}/{len(grades)}")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--jobs-dir", type=Path, default=REPO / "jobs")
    parser.add_argument(
        "--job", help="Job directory name (default: newest with layman trials)"
    )
    args = parser.parse_args(argv)

    jobs_dir: Path = args.jobs_dir
    if not jobs_dir.is_dir():
        print(f"No jobs directory: {jobs_dir}", file=sys.stderr)
        return 1

    job_dir = jobs_dir / args.job if args.job else latest_job(jobs_dir)
    if job_dir is None or not job_dir.is_dir():
        print("No job with layman-intent trials found", file=sys.stderr)
        return 1

    grades = [
        Grade(spec, grade_trajectory(spec.task_id, trajectory))
        for spec, trajectory in find_trials(job_dir)
    ]
    if not grades:
        print(f"No layman-intent trials in {job_dir}", file=sys.stderr)
        return 1

    print(f"Job: {job_dir.name}")
    print(render_report(grades))
    return 0 if all(result.passed for result in grades) else 1


if __name__ == "__main__":
    raise SystemExit(main())
