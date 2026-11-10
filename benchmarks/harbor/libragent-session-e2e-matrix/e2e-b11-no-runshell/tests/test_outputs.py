"""B-11: on a Windows host the agent must not use workspace__runShell.

Checks are tool-call based (issue #2655): the instruction text itself
mentions ``workspace__runShell``, so blob matching over the whole
trajectory false-positives on the agent merely quoting the tool name.
Semantics:

* no *successful* ``workspace__runShell`` call (an attempted call that
  errored out is acceptable — the instruction tells the agent to try it);
* ``submitInteractiveShellInput`` must never be called at all;
* at least one successful PowerShell call with non-empty output.
"""

from __future__ import annotations

from verifier_lib import (
    INTERACTIVE_SUBMIT_SUFFIX,
    POWERSHELL_SUFFIXES,
    RUN_SHELL_SUFFIX,
    call_function_name,
    call_matches,
    load_trajectory,
    observation_results_by_call_id,
    result_content_text,
    result_succeeded,
    tool_calls,
    trajectory_steps,
)


def _load() -> tuple[list[dict], dict[str, dict]]:
    steps = trajectory_steps(load_trajectory())
    return tool_calls(steps), observation_results_by_call_id(steps)


def test_trajectory_harvested():
    trajectory = load_trajectory()
    assert trajectory.get("steps"), (
        "no trajectory steps at /logs/agent/trajectory.json — the agent "
        "session was not harvested, so tool-call evidence is missing"
    )


def test_no_successful_runshell_call():
    calls, results = _load()
    offenders = []
    for call in calls:
        if not call_matches(call, RUN_SHELL_SUFFIX):
            continue
        result = results.get(str(call.get("tool_call_id") or ""))
        if result is not None and result_succeeded(result):
            offenders.append(call_function_name(call))
    assert not offenders, (
        f"workspace__runShell succeeded ({offenders}) — on Windows it must "
        "be unavailable, so only failed attempts are acceptable"
    )


def test_no_submit_interactive_shell_input_call():
    calls, _ = _load()
    offenders = [
        call_function_name(call)
        for call in calls
        if call_matches(call, INTERACTIVE_SUBMIT_SUFFIX)
    ]
    assert not offenders, (
        f"submitInteractiveShellInput was called ({offenders}) — the tool "
        "must not exist in the tool surface (B-11 regression guard)"
    )


def test_powershell_tool_used_with_output():
    calls, results = _load()
    used = []
    for call in calls:
        if not call_matches(call, POWERSHELL_SUFFIXES):
            continue
        result = results.get(str(call.get("tool_call_id") or ""))
        if (
            result is not None
            and result_succeeded(result)
            and result_content_text(result).strip()
        ):
            used.append(call_function_name(call))
    assert used, (
        "no successful PowerShell tool call with non-empty output "
        "(workspace__runPowerShell / workspace__runInPersistentPowerShell)"
    )
