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
import shlex
from dataclasses import dataclass
from pathlib import Path, PurePath

SCRIPT_LAUNCHERS = frozenset(
    {"python", "python3", "uv", "bash", "sh", "zsh", "node", "pwsh", "powershell"}
)
SHELLS = frozenset({"bash", "sh", "zsh"})
# Prefixes that run the next command: (name, options that take a value, positional args)
WRAPPERS: dict[str, tuple[frozenset[str], int]] = {
    "timeout": (frozenset({"-s", "--signal", "-k", "--kill-after"}), 1),
    "sudo": (frozenset({"-u", "-g", "-C", "-D"}), 0),
    "nohup": (frozenset(), 0),
    "time": (frozenset(), 0),
    "env": (frozenset({"-u", "-C"}), 0),
    "exec": (frozenset({"-a"}), 0),
    "command": (frozenset(), 0),
}
OPERATOR_CHARS = ";&|"
MAX_SHELL_NESTING = 3


def _strip_prefixes(tokens: list[str]) -> list[str]:
    """Drop ``VAR=value`` assignments and wrapper commands such as ``timeout 60``."""
    while tokens:
        head = tokens[0]
        if "=" in head and not head.startswith("-"):
            tokens = tokens[1:]
            continue
        wrapper = WRAPPERS.get(PurePath(head).name)
        if wrapper is None:
            return tokens
        valued_options, positional = wrapper
        tokens = tokens[1:]
        while tokens and tokens[0].startswith("-"):
            option = tokens.pop(0)
            if option in valued_options and tokens:
                tokens.pop(0)
        tokens = tokens[positional:]
    return tokens


def _shell_payload(tokens: list[str]) -> str | None:
    """Command string of ``bash -c '<cmd>'`` (also ``-lc``), else None."""
    for index, token in enumerate(tokens[1:], start=1):
        if token.startswith("-") and not token.startswith("--") and "c" in token[1:]:
            return tokens[index + 1] if index + 1 < len(tokens) else None
    return None


def _segments(command: str) -> list[list[str]]:
    """Split a shell command into simple commands, respecting quotes."""
    segments: list[list[str]] = []
    for line in command.replace("\\", "/").splitlines():  # shlex eats backslashes
        lexer = shlex.shlex(line, posix=True, punctuation_chars=OPERATOR_CHARS)
        lexer.whitespace_split = True
        try:
            tokens = list(lexer)
        except ValueError:
            tokens = line.split()
        current: list[str] = []
        for token in tokens:
            if token and set(token) <= set(OPERATOR_CHARS):
                segments.append(current)
                current = []
            else:
                current.append(token)
        segments.append(current)
    return segments


def runs_script(command: str, scripts: tuple[str, ...], _depth: int = 0) -> bool:
    """True when a command segment executes one of ``scripts``.

    The script must be the program itself or an argument of an interpreter,
    after wrappers like ``timeout``/``nohup``/``sudo``; ``bash -c '<cmd>'`` is
    checked recursively. ``cat deploy_skill.py`` or ``grep x verify_setup.sh``
    does not count.
    """
    for tokens in _segments(command):
        tokens = _strip_prefixes(tokens)
        if not tokens:
            continue
        names = [PurePath(token).name for token in tokens]
        if names[0] in scripts:
            return True
        if names[0] in SHELLS and _depth < MAX_SHELL_NESTING:
            payload = _shell_payload(tokens)
            if payload is not None:
                if runs_script(payload, scripts, _depth + 1):
                    return True
                continue
        if names[0] in SCRIPT_LAUNCHERS and any(name in scripts for name in names):
            return True
    return False


def calls_wake(command: str) -> bool:
    """True when a stored command calls the session wake URL.

    ``"$LIBRAGENT_WAKE_URL"`` in double quotes is expanded by the shell that runs
    the subscribe call, so the stored text then holds the ``/wake/<token>`` URL.
    """
    return "LIBRAGENT_WAKE_URL" in command or "/wake/" in command


@dataclass(frozen=True)
class Step:
    tool: str  # matched as a suffix of the namespaced tool name
    args_contain: str | None = None  # substring of the serialized arguments
    scripts: tuple[str, ...] = ()  # shell steps: one of these must be executed
    min_count: int = 1  # matching calls required (the last one must succeed)
    alternatives: tuple[Step, ...] = ()  # any of these also satisfies the step

    @property
    def label(self) -> str:
        detail = " ".join(
            part for part in (" | ".join(self.scripts), self.args_contain) if part
        )
        own = f"{self.tool} ({detail})" if detail else self.tool
        if self.min_count > 1:
            own = f"{own} x{self.min_count}"
        return " or ".join([own, *(alt.label for alt in self.alternatives)])

    def matches(self, call: Call) -> bool:
        if any(alt.matches(call) for alt in self.alternatives):
            return True
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
# Catalog installs look the preset up first; direct npm installs (T07, T10) do not.
INSTALL_MCP_FROM_CATALOG = (Step("__listServers", "catalog"), *INSTALL_MCP)

TASKS: dict[str, TaskSpec] = {
    spec.task_id: spec
    for spec in (
        TaskSpec("T01", "t01-water-reminder", "P", "loop", SCHEDULE_CALLBACK),
        TaskSpec(
            "T02",
            "t02-watch-folder-mornings",
            "P",
            "loop",
            (
                # A scheduled task runs outside this session, so it only reaches
                # the folder when the call carries a workspaceOverride.
                Step(
                    "__scheduleCallback",
                    alternatives=(
                        Step("__createScheduledTask", '"workspaceOverride"'),
                    ),
                ),
            ),
        ),
        TaskSpec(
            "T03",
            "t03-weekday-summary",
            "P",
            "schedule",
            (Step("__createScheduledTask"),),
        ),
        TaskSpec(
            "T04",
            "t04-event-subscribe",
            "P",
            "call-me-back",
            # An external event can't be waited on in-turn: the session must hook
            # the mock CLI up to the wake URL and go idle. Harbor ends the trial at
            # idle, so the firing is not judged; the test checks the recorded hook.
            (Step("__runShell", "subscribe", scripts=("relayctl",)),),
        ),
        TaskSpec("T05", "t05-tell-me-later", "P", "loop", SCHEDULE_CALLBACK),
        TaskSpec(
            "T06", "t06-stock-data", "P", "mcp-installer", INSTALL_MCP_FROM_CATALOG
        ),
        TaskSpec("T07", "t07-connect-npx-package", "P", "mcp-installer", INSTALL_MCP),
        TaskSpec(
            "T08",
            "t08-install-github-skill",
            "P",
            "skill-deployer",
            (Step("__runShell", scripts=("deploy_skill.py",)),),
        ),
        TaskSpec(
            "T09", "t09-install-github-mcp", "P", "mcp-installer", INSTALL_MCP_FROM_CATALOG
        ),
        TaskSpec("T10", "t10-install-that", "P", "mcp-installer", INSTALL_MCP),
        TaskSpec(
            "T11",
            "t11-command-not-found",
            "P",
            "setup-wizard",
            (
                Step(
                    "__runShell",
                    scripts=(
                        "verify_setup.sh",
                        "verify_setup.ps1",
                        "install_node.sh",
                        "install_node.ps1",
                    ),
                ),
            ),
        ),
        TaskSpec(
            "T12",
            "t12-save-organizing-routine",
            "P",
            "skill-creator",
            (
                Step("__writeFile", "SKILL.md"),
                Step(
                    "__runShell",
                    "--strict",
                    scripts=("validate_skill.py",),
                    # skill.py validate is always strict; deploy validates first.
                    alternatives=(
                        Step("__runShell", "validate", scripts=("skill.py",)),
                        Step("__runShell", "deploy", scripts=("skill.py",)),
                    ),
                ),
            ),
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
            (Step("__spawnSession", min_count=2),),
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
                # The seed has epochs 1 and 2; the answer is in the highest one.
                Step("__readFile", "pre_compaction_epoch_2"),
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
        if len(indexes) < step.min_count:
            failures.append(f"{step.label}: {len(indexes)} call(s)")
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
