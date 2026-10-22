#!/usr/bin/env python3
"""Grade layman-intent Harbor trials by skill routing in trajectory.json (#2183).

Each trial directory name must contain its task id (e.g. ``t01-water-reminder``).
For every trial the checker reads ATIF ``trajectory.json`` and verifies:

- the expected bundled skill was read (``workspace__readFile`` on ``<name>/SKILL.md``),
- required tool calls happened and forbidden ones did not (matched by ``__suffix``),
- argument checks (e.g. ``delaySeconds`` close to 600),
- Q tasks end with a question and N tasks spawn no sub-sessions.

Trials whose run failed with an empty-work error are still graded: a correct
"ask first" reply has no tool calls but the trajectory is written.

Usage:
  scripts/check-layman-intent-trajectories.py [--jobs-dir jobs] [--job <name>]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import defaultdict
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]

TASK_ID_RE = re.compile(r"(?:^|[^a-z0-9])(t\d{2})(?:[^0-9]|$)", re.IGNORECASE)
SKILL_PATH_RE = re.compile(r"([A-Za-z0-9_.-]+)/SKILL\.md\b")
QUESTION_RE = re.compile(r"[?？]")

ArgPredicate = Callable[[dict[str, object]], bool]


@dataclass(frozen=True)
class ArgCheck:
    tool: str
    description: str
    predicate: ArgPredicate


@dataclass(frozen=True)
class Expectation:
    task_id: str
    category: str  # P (proceed), Q (must ask), N (must not over-trigger)
    skill: str | None
    must_call: tuple[str, ...] = ()
    must_not_call: tuple[str, ...] = ()
    arg_checks: tuple[ArgCheck, ...] = ()


def _number(args: dict[str, object], key: str) -> float | None:
    value = args.get(key)
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        return float(value)
    if isinstance(value, str):
        try:
            return float(value)
        except ValueError:
            return None
    return None


def _delay_near(seconds: float, tolerance: float) -> ArgPredicate:
    def check(args: dict[str, object]) -> bool:
        delay = _number(args, "delaySeconds")
        return delay is not None and abs(delay - seconds) <= tolerance

    return check


def _has_cron(args: dict[str, object]) -> bool:
    cron = args.get("cronExpression")
    return isinstance(cron, str) and bool(cron.strip())


SCHEDULING = ("__scheduleCallback", "__createScheduledTask")
# Skill deploys run deploy_skill.py through the shell, so an "ask first" turn must not shell out.
INSTALLS = ("__registerServer", "__runShell")
SUB_SESSIONS = ("__spawnSession",)

P0_EXPECTATIONS: dict[str, Expectation] = {
    e.task_id: e
    for e in (
        Expectation(
            "T01",
            "P",
            "loop",
            must_call=("__scheduleCallback",),
            must_not_call=("__createScheduledTask",),
            arg_checks=(
                ArgCheck(
                    "__scheduleCallback", "delaySeconds≈600", _delay_near(600, 60)
                ),
            ),
        ),
        Expectation(
            "T02",
            "P",
            "loop",
            must_call=("__scheduleCallback",),
            must_not_call=("__createScheduledTask",),
            arg_checks=(
                ArgCheck("__scheduleCallback", "cronExpression set", _has_cron),
            ),
        ),
        Expectation(
            "T03",
            "P",
            "schedule",
            must_call=("__createScheduledTask",),
            must_not_call=("__scheduleCallback",),
        ),
        Expectation(
            "T04",
            "P",
            "call-me-back",
            must_call=("__spawnProcess", "__waitForProcess"),
            must_not_call=("__scheduleCallback",),
        ),
        Expectation("T05", "Q", None, must_not_call=SCHEDULING),
        Expectation("T06", "P", "mcp-installer", must_call=("__registerServer",)),
        Expectation(
            "T07",
            "P",
            "mcp-installer",
            must_call=("__registerServer", "__listServers"),
        ),
        Expectation(
            "T08",
            "P",
            "skill-deployer",
            must_not_call=("__registerServer",),
        ),
        Expectation(
            "T09",
            "P",
            "mcp-installer",
            must_call=("__registerServer",),
        ),
        Expectation("T10", "Q", None, must_not_call=INSTALLS),
        Expectation("T11", "P", "setup-wizard", must_not_call=("__registerServer",)),
        Expectation("T12", "P", "skill-creator"),
        Expectation(
            "T13",
            "P",
            "delegate",
            must_call=("__spawnSession", "__checkSession"),
        ),
        Expectation("T14", "P", "divide-conquer", must_call=("__spawnSession",)),
        Expectation("T15", "N", None, must_not_call=SUB_SESSIONS),
        Expectation(
            "T16",
            "P",
            "teamwork",
            must_call=("__prepareTeamworkWorkspace",),
        ),
        Expectation("T17", "P", "recruit", must_call=("__createAgent",)),
        Expectation("T18", "P", "context-recall"),
        Expectation(
            "T19",
            "P",
            "knowledge-distiller",
            must_call=("__recordKnowledge",),
        ),
    )
}

# Skill aliases kept so trials recorded before a rename still grade correctly.
SKILL_ALIASES = {"tool-installer": "mcp-installer"}


@dataclass
class ToolCall:
    name: str
    arguments: dict[str, object]


@dataclass
class Trace:
    tool_calls: list[ToolCall] = field(default_factory=list)
    final_agent_message: str = ""

    def calls(self, suffix: str) -> list[ToolCall]:
        return [call for call in self.tool_calls if call.name.endswith(suffix)]

    def skills_read(self) -> set[str]:
        skills: set[str] = set()
        for call in self.calls("__readFile"):
            for value in call.arguments.values():
                if isinstance(value, str):
                    for match in SKILL_PATH_RE.finditer(value):
                        name = match.group(1)
                        skills.add(SKILL_ALIASES.get(name, name))
        return skills


def _parse_arguments(raw: object) -> dict[str, object]:
    if isinstance(raw, dict):
        return {str(key): value for key, value in raw.items()}
    if isinstance(raw, str):
        try:
            parsed = json.loads(raw)
        except json.JSONDecodeError:
            return {}
        if isinstance(parsed, dict):
            return {str(key): value for key, value in parsed.items()}
    return {}


def load_trace(path: Path) -> Trace:
    data = json.loads(path.read_text(encoding="utf-8", errors="replace"))
    trace = Trace()
    steps = data.get("steps") if isinstance(data, dict) else None
    for step in steps or []:
        if not isinstance(step, dict):
            continue
        for call in step.get("tool_calls") or []:
            if not isinstance(call, dict):
                continue
            name = call.get("function_name")
            if isinstance(name, str):
                trace.tool_calls.append(
                    ToolCall(
                        name=name, arguments=_parse_arguments(call.get("arguments"))
                    )
                )
        message = step.get("message")
        if (
            step.get("source") == "agent"
            and isinstance(message, str)
            and message.strip()
        ):
            trace.final_agent_message = message
    return trace


@dataclass
class Grade:
    task_id: str
    skill: str | None
    category: str
    failures: list[str]

    @property
    def passed(self) -> bool:
        return not self.failures


def grade(expectation: Expectation, trace: Trace) -> Grade:
    failures: list[str] = []

    if expectation.skill and expectation.skill not in trace.skills_read():
        read = sorted(trace.skills_read()) or ["none"]
        failures.append(f"skill {expectation.skill} not read (read: {', '.join(read)})")

    for suffix in expectation.must_call:
        if not trace.calls(suffix):
            failures.append(f"missing call {suffix}")

    for suffix in expectation.must_not_call:
        if trace.calls(suffix):
            failures.append(f"forbidden call {suffix}")

    for check in expectation.arg_checks:
        calls = trace.calls(check.tool)
        if calls and not any(check.predicate(call.arguments) for call in calls):
            failures.append(f"{check.tool}: expected {check.description}")

    if expectation.category == "Q" and not QUESTION_RE.search(
        trace.final_agent_message
    ):
        failures.append("Q task: final reply does not ask a question")

    return Grade(expectation.task_id, expectation.skill, expectation.category, failures)


def task_id_for(trial_dir: Path) -> str | None:
    match = TASK_ID_RE.search(trial_dir.name)
    return match.group(1).upper() if match else None


def find_trials(job_dir: Path) -> list[tuple[str, Path]]:
    trials: list[tuple[str, Path]] = []
    for trajectory in sorted(job_dir.rglob("trajectory.json")):
        for parent in trajectory.parents:
            if parent == job_dir.parent:
                break
            task_id = task_id_for(parent)
            if task_id:
                trials.append((task_id, trajectory))
                break
    return trials


def latest_job(jobs_dir: Path) -> Path | None:
    jobs = sorted(
        (path for path in jobs_dir.iterdir() if path.is_dir()),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    for job in jobs:
        if find_trials(job):
            return job
    return None


def render_report(grades: list[Grade]) -> str:
    lines = ["Per-task:"]
    for result in sorted(grades, key=lambda g: g.task_id):
        status = "PASS" if result.passed else "FAIL"
        label = result.skill or f"({result.category})"
        detail = "" if result.passed else " — " + "; ".join(result.failures)
        lines.append(f"  {result.task_id} {label:<22} {status}{detail}")

    by_skill: dict[str, list[bool]] = defaultdict(list)
    for result in grades:
        by_skill[result.skill or f"({result.category})"].append(result.passed)
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
        print("No job with layman-intent trials (tNN-*) found", file=sys.stderr)
        return 1

    grades: list[Grade] = []
    unknown: list[str] = []
    for task_id, trajectory in find_trials(job_dir):
        expectation = P0_EXPECTATIONS.get(task_id)
        if expectation is None:
            unknown.append(task_id)
            continue
        try:
            trace = load_trace(trajectory)
        except (OSError, json.JSONDecodeError) as error:
            grades.append(
                Grade(task_id, expectation.skill, expectation.category, [str(error)])
            )
            continue
        grades.append(grade(expectation, trace))

    if not grades:
        print(f"No gradable trials in {job_dir}", file=sys.stderr)
        return 1

    print(f"Job: {job_dir.name}")
    print(render_report(grades))
    if unknown:
        print(f"\nNo expectation for: {', '.join(sorted(set(unknown)))}")
    return 0 if all(result.passed for result in grades) else 1


if __name__ == "__main__":
    raise SystemExit(main())
