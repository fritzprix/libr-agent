"""2672 M-6 (Windows): stdio MCP server args with &, |, ^, <, > pass through without cmd.exe."""

import json
from pathlib import Path

from verifier_lib import load_trajectory, require_session_evidence

EXPECTED_ARG = "a&b|c^d<e>f"
RECEIVED_FILE = Path("/app/m6_received_args.json")


def test_stdio_metachar_arg_passed_verbatim():
    require_session_evidence()
    assert RECEIVED_FILE.is_file(), (
        "m6_received_args.json missing — stdio MCP server helper was not spawned"
    )
    args = json.loads(RECEIVED_FILE.read_text(encoding="utf-8"))
    assert isinstance(args, list) and EXPECTED_ARG in args, (
        f"expected verbatim metachar arg {EXPECTED_ARG!r} without cmd.exe mangling, got {args!r}"
    )
    traj_blob = json.dumps(load_trajectory())
    assert "registerServer" in traj_blob, "expected tool__registerServer call in trajectory"
