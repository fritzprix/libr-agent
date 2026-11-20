"""2672 M-6 (Windows): stdio MCP server args with &, |, ^, <, > pass through without cmd.exe.

Checks are tool-call based (matching e2e-b11-no-runshell):
- exactly one ``registerServer`` tool call whose ``transport.args`` contains
  the verbatim metacharacter string ``a&b|c^d<e>f``;
- no successful shell/PowerShell call referencing ``m6_echo_args.py`` (prevents
  faking the output file without going through the stdio transport);
- ``/app/m6_received_args.json`` written by the spawned stdio child contains
  ``a&b|c^d<e>f`` verbatim.
"""

from __future__ import annotations

import json
from pathlib import Path

from verifier_lib import (
    POWERSHELL_SUFFIXES,
    RUN_SHELL_SUFFIX,
    call_matches,
    load_trajectory,
    observation_results_by_call_id,
    result_succeeded,
    tool_calls,
    trajectory_steps,
)

EVIDENCE = Path("/logs/agent/session_e2e_evidence.json")
EXPECTED_ARG = "a&b|c^d<e>f"
RECEIVED_FILE = Path("/app/m6_received_args.json")
SHELL_SUFFIXES = POWERSHELL_SUFFIXES | {RUN_SHELL_SUFFIX}


def _parse_arguments(call: dict) -> dict:
    raw = call.get("arguments")
    if isinstance(raw, dict):
        return raw
    if isinstance(raw, str):
        try:
            parsed = json.loads(raw)
            if isinstance(parsed, dict):
                return parsed
        except ValueError:
            pass
    return {}


def test_stdio_metachar_arg_passed_verbatim():
    assert EVIDENCE.is_file() and json.loads(EVIDENCE.read_text(encoding="utf-8")).get("sessionId"), (
        "missing sessionId in E2E evidence"
    )
    steps = trajectory_steps(load_trajectory())
    assert steps, "no trajectory steps harvested"

    calls = tool_calls(steps)
    results = observation_results_by_call_id(steps)

    # 1. Exactly one registerServer call with EXPECTED_ARG in transport.args.
    register_calls = [c for c in calls if call_matches(c, "registerserver")]
    matching_registers = []
    for call in register_calls:
        args = _parse_arguments(call)
        transport = args.get("transport") if isinstance(args.get("transport"), dict) else {}
        t_args = transport.get("args")
        if (
            transport.get("type") == "stdio"
            and isinstance(t_args, list)
            and EXPECTED_ARG in t_args
        ):
            matching_registers.append(call)
    assert len(matching_registers) == 1, (
        f"expected exactly 1 stdio tool__registerServer call with {EXPECTED_ARG!r} in "
        f"transport.args, found {len(matching_registers)} (total registerServer={len(register_calls)})"
    )

    # 2. No successful shell/PowerShell call referencing m6_echo_args.py.
    for call in calls:
        if not call_matches(call, SHELL_SUFFIXES):
            continue
        raw_args = json.dumps(call.get("arguments") or "")
        if "m6_echo_args.py" not in raw_args.lower():
            continue
        result = results.get(str(call.get("tool_call_id") or ""))
        assert result is None or not result_succeeded(result), (
            "shell/PowerShell tool ran m6_echo_args.py directly instead of letting "
            "the MCP stdio transport spawn it"
        )

    # 3. Spawn proof: child received the verbatim metacharacter argument.
    assert RECEIVED_FILE.is_file(), (
        "m6_received_args.json missing — stdio MCP server helper was not spawned"
    )
    received = json.loads(RECEIVED_FILE.read_text(encoding="utf-8"))
    assert isinstance(received, list) and EXPECTED_ARG in received, (
        f"expected verbatim metachar arg {EXPECTED_ARG!r} without cmd.exe mangling, got {received!r}"
    )
