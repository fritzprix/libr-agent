"""2565 O-7 (Windows): docs skill doctor.ps1 reports 'Doctor: OK' with no junction loop.

Checks are tool-call based (matching e2e-b11-no-runshell): the verifier inspects
actual PowerShell tool calls and their ATIF observation results rather than a
model-written output file.
"""

from __future__ import annotations

import json
from pathlib import Path

from verifier_lib import (
    POWERSHELL_SUFFIXES,
    call_matches,
    load_trajectory,
    observation_results_by_call_id,
    result_content_text,
    result_succeeded,
    tool_calls,
    trajectory_steps,
)

EVIDENCE = Path("/logs/agent/session_e2e_evidence.json")


def _call_arguments_text(call: dict) -> str:
    args = call.get("arguments")
    if isinstance(args, str):
        return args
    if isinstance(args, dict):
        return json.dumps(args)
    return ""


def test_docs_doctor_ps1_ok_without_junction_loop():
    assert EVIDENCE.is_file() and json.loads(EVIDENCE.read_text(encoding="utf-8")).get("sessionId"), (
        "missing sessionId in E2E evidence"
    )
    steps = trajectory_steps(load_trajectory())
    assert steps, "no trajectory steps harvested"

    calls = tool_calls(steps)
    results = observation_results_by_call_id(steps)

    matching_outputs: list[str] = []
    for call in calls:
        if not call_matches(call, POWERSHELL_SUFFIXES):
            continue
        if "doctor.ps1" not in _call_arguments_text(call).lower():
            continue
        result = results.get(str(call.get("tool_call_id") or ""))
        if result is not None and result_succeeded(result):
            matching_outputs.append(result_content_text(result))

    assert matching_outputs, (
        "no successful PowerShell tool call executing doctor.ps1 was observed in trajectory"
    )
    assert any("Doctor: OK" in out for out in matching_outputs), (
        f"expected 'Doctor: OK' in doctor.ps1 tool output, got: {matching_outputs!r}"
    )
    for out in matching_outputs:
        lower = out.lower()
        assert "os error 448" not in lower and "untrusted mount point" not in lower, (
            f"NTFS junction/mount error detected in doctor.ps1 output: {out[:500]!r}"
        )
