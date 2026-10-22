"""Critical-path grading for the layman-intent suite (#2183).

A task passes when the trajectory contains each step of its skill's critical
path and the last call matching each step succeeded. N tasks pass when none of
their forbidden tools were called. Nothing else is judged here; deeper analysis
belongs to an agent-based eval loop.

This file is the source of truth. Each task's ``tests/critical_path.py`` is a
byte-for-byte copy so the container verifier can grade
``/logs/agent/trajectory.json`` without the repo; a unit test keeps them equal.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Step:
    tool: str  # matched as a suffix of the namespaced tool name
    args_contain: str | None = None  # substring of the serialized arguments

    def matches(self, call: Call) -> bool:
        if not call.name.endswith(self.tool):
            return False
        return self.args_contain is None or self.args_contain in call.arguments


@dataclass(frozen=True)
class TaskSpec:
    task_id: str
    directory: str
    category: str  # P (proceed) or N (must not over-trigger)
    skill: str | None
    critical_path: tuple[Step, ...] = ()
    forbidden: tuple[str, ...] = ()

    @property
    def harbor_name(self) -> str:
        return f"fritzprix/layman-{self.directory}-v1"


@dataclass(frozen=True)
class Call:
    name: str
    arguments: str
    succeeded: bool | None  # None: no result recorded


SCHEDULE_CALLBACK = (Step("__scheduleCallback"),)
INSTALL_MCP = (Step("__registerServer"), Step("__verifyServer"))

TASKS: dict[str, TaskSpec] = {
    spec.task_id: spec
    for spec in (
        TaskSpec("T01", "t01-water-reminder", "P", "loop", SCHEDULE_CALLBACK),
        TaskSpec("T02", "t02-watch-folder-mornings", "P", "loop", SCHEDULE_CALLBACK),
        TaskSpec(
            "T03",
            "t03-weekday-summary",
            "P",
            "schedule",
            (Step("__createScheduledTask"),),
        ),
        TaskSpec(
            "T04",
            "t04-build-done-notify",
            "P",
            "call-me-back",
            (Step("__spawnProcess"), Step("__waitForProcess")),
        ),
        TaskSpec("T05", "t05-tell-me-later", "P", "loop", SCHEDULE_CALLBACK),
        TaskSpec("T06", "t06-stock-data", "P", "mcp-installer", INSTALL_MCP),
        TaskSpec("T07", "t07-connect-npx-package", "P", "mcp-installer", INSTALL_MCP),
        TaskSpec(
            "T08",
            "t08-install-github-skill",
            "P",
            "skill-deployer",
            (Step("__runShell", "deploy_skill.py"),),
        ),
        TaskSpec("T09", "t09-install-github-mcp", "P", "mcp-installer", INSTALL_MCP),
        TaskSpec("T10", "t10-install-that", "P", "mcp-installer", INSTALL_MCP),
        TaskSpec(
            "T11",
            "t11-command-not-found",
            "P",
            "setup-wizard",
            (Step("__runShell", "verify_setup"),),
        ),
        TaskSpec(
            "T12",
            "t12-save-organizing-routine",
            "P",
            "skill-creator",
            (Step("__runShell", "validate_skill.py"),),
        ),
        TaskSpec(
            "T13",
            "t13-hand-off-task",
            "P",
            "delegate",
            (Step("__spawnSession"), Step("__checkSession")),
        ),
        TaskSpec(
            "T14",
            "t14-summarize-50-docs",
            "P",
            "divide-conquer",
            (Step("__spawnSession"),),
        ),
        TaskSpec(
            "T15",
            "t15-readme-typo",
            "N",
            None,
            forbidden=("__spawnSession",),
        ),
        TaskSpec(
            "T16",
            "t16-build-a-team",
            "P",
            "teamwork",
            (Step("__prepareTeamworkWorkspace"),),
        ),
        TaskSpec(
            "T17",
            "t17-stock-assistant",
            "P",
            "recruit",
            (Step("__createAgent"),),
        ),
        TaskSpec(
            "T18",
            "t18-recall-deploy-time",
            "P",
            "context-recall",
            (
                Step("__globFiles", "pre_compaction_epoch_"),
                Step("__readFile", "pre_compaction_epoch_"),
            ),
        ),
        TaskSpec(
            "T19",
            "t19-remember-today",
            "P",
            "knowledge-distiller",
            (Step("__recordKnowledge"),),
        ),
    )
}


def _arguments_text(raw: object) -> str:
    if isinstance(raw, str):
        return raw
    return json.dumps(raw, ensure_ascii=False, sort_keys=True)


def _result_succeeded(result: dict[str, object]) -> bool | None:
    extra = result.get("extra")
    if isinstance(extra, dict):
        tool_error = extra.get("tool_error")
        if isinstance(tool_error, bool):
            return not tool_error
    return None


def load_calls(trajectory: object) -> list[Call]:
    """Tool calls in order, each paired with its observation's success flag."""
    if not isinstance(trajectory, dict):
        return []
    steps = trajectory.get("steps")
    if not isinstance(steps, list):
        return []

    outcomes: dict[str, bool | None] = {}
    pending: list[tuple[str, str, str]] = []
    for step in steps:
        if not isinstance(step, dict):
            continue
        for call in step.get("tool_calls") or []:
            if not isinstance(call, dict):
                continue
            name = call.get("function_name")
            call_id = call.get("tool_call_id")
            if isinstance(name, str):
                pending.append(
                    (
                        call_id if isinstance(call_id, str) else "",
                        name,
                        _arguments_text(call.get("arguments")),
                    )
                )
        observation = step.get("observation")
        results = observation.get("results") if isinstance(observation, dict) else None
        for result in results or []:
            if isinstance(result, dict) and isinstance(
                result.get("source_call_id"), str
            ):
                outcomes[result["source_call_id"]] = _result_succeeded(result)

    return [
        Call(name=name, arguments=arguments, succeeded=outcomes.get(call_id))
        for call_id, name, arguments in pending
    ]


def grade_calls(spec: TaskSpec, calls: list[Call]) -> list[str]:
    """Return failure reasons; an empty list means PASS."""
    failures: list[str] = []
    for step in spec.critical_path:
        matching = [call for call in calls if step.matches(call)]
        label = step.tool + (f" ({step.args_contain})" if step.args_contain else "")
        if not matching:
            failures.append(f"missing {label}")
        elif matching[-1].succeeded is not True:
            state = "failed" if matching[-1].succeeded is False else "has no result"
            failures.append(f"last {label} {state}")
    for tool in spec.forbidden:
        if any(call.name.endswith(tool) for call in calls):
            failures.append(f"forbidden {tool}")
    return failures


def grade_trajectory(task_id: str, path: Path) -> list[str]:
    try:
        trajectory = json.loads(path.read_text(encoding="utf-8", errors="replace"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"cannot read trajectory: {error}"]
    return grade_calls(TASKS[task_id], load_calls(trajectory))
