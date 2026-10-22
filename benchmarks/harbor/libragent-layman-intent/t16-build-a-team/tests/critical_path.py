"""Critical-path grading for the layman-intent suite (#2183).

A task passes when the trajectory contains each step of its skill's critical
path in order and the last call matching each step succeeded. N tasks pass
when none of their forbidden tools were called. Nothing else is judged here; deeper analysis
belongs to an agent-based eval loop.

This file is the source of truth. Each task's ``tests/critical_path.py`` is a
byte-for-byte copy so the container verifier can grade
``/logs/agent/trajectory.json`` without the repo; a unit test keeps them equal.
"""

from __future__ import annotations

import json
import re
import shlex
from dataclasses import dataclass
from pathlib import Path, PurePath

SCRIPT_LAUNCHERS = frozenset(
    {"python", "python3", "uv", "bash", "sh", "zsh", "node", "pwsh", "powershell", "&"}
)
COMMAND_SEPARATORS = re.compile(r"&&|\|\||[;|\n]")


def runs_script(command: str, scripts: tuple[str, ...]) -> bool:
    """True when a command segment executes one of ``scripts``.

    The script must be the program itself or an argument of an interpreter, so
    ``cat deploy_skill.py`` or ``grep x verify_setup.sh`` does not count.
    """
    for segment in COMMAND_SEPARATORS.split(command):
        segment = segment.replace("\\", "/")  # Windows paths; shlex eats backslashes
        try:
            tokens = shlex.split(segment)
        except ValueError:
            tokens = segment.split()
        while tokens and "=" in tokens[0] and not tokens[0].startswith("-"):
            tokens = tokens[1:]
        if not tokens:
            continue
        names = [PurePath(token).name for token in tokens]
        if names[0] in scripts:
            return True
        if names[0] in SCRIPT_LAUNCHERS and any(name in scripts for name in names):
            return True
    return False


@dataclass(frozen=True)
class Step:
    tool: str  # matched as a suffix of the namespaced tool name
    args_contain: str | None = None  # substring of the serialized arguments
    scripts: tuple[str, ...] = ()  # shell steps: one of these must be executed

    @property
    def label(self) -> str:
        detail = self.args_contain or " | ".join(self.scripts)
        return f"{self.tool} ({detail})" if detail else self.tool

    def matches(self, call: Call) -> bool:
        if not call.name.endswith(self.tool):
            return False
        if self.args_contain is not None and self.args_contain not in call.arguments:
            return False
        return not self.scripts or runs_script(call.shell_text, self.scripts)


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

    @property
    def shell_text(self) -> str:
        """``command`` or ``script`` argument of a shell call."""
        try:
            parsed = json.loads(self.arguments)
        except json.JSONDecodeError:
            return ""
        if not isinstance(parsed, dict):
            return ""
        return "\n".join(
            value
            for key in ("command", "script")
            if isinstance(value := parsed.get(key), str)
        )


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
            (Step("__runShell", scripts=("deploy_skill.py",)),),
        ),
        TaskSpec("T09", "t09-install-github-mcp", "P", "mcp-installer", INSTALL_MCP),
        TaskSpec("T10", "t10-install-that", "P", "mcp-installer", INSTALL_MCP),
        TaskSpec(
            "T11",
            "t11-command-not-found",
            "P",
            "setup-wizard",
            (Step("__runShell", scripts=("verify_setup.sh", "verify_setup.ps1")),),
        ),
        TaskSpec(
            "T12",
            "t12-save-organizing-routine",
            "P",
            "skill-creator",
            (Step("__runShell", scripts=("validate_skill.py",)),),
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
    previous: tuple[str, int] | None = None  # label and first index of prior step
    for step in spec.critical_path:
        indexes = [i for i, call in enumerate(calls) if step.matches(call)]
        if not indexes:
            failures.append(f"missing {step.label}")
            continue
        last = calls[indexes[-1]]
        if previous is not None and indexes[-1] < previous[1]:
            failures.append(f"{step.label} only before {previous[0]}")
        elif last.succeeded is not True:
            state = "failed" if last.succeeded is False else "has no result"
            failures.append(f"last {step.label} {state}")
        previous = (step.label, indexes[0])
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
