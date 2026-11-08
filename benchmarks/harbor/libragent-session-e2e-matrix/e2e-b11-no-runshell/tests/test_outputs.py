import json
from pathlib import Path

TRAJECTORY = Path("/logs/agent/trajectory.json")


def test_no_runshell_tool_call():
    traj = json.loads(TRAJECTORY.read_text(encoding="utf-8"))
    blob = json.dumps(traj).lower()
    assert "runshell" not in blob or "not available" in blob
    assert "submitinteractiveshellinput" not in blob


def test_powershell_or_persistent_used():
    blob = json.dumps(json.loads(TRAJECTORY.read_text(encoding="utf-8"))).lower()
    assert "runinpersistentpowershell" in blob or "runpowershell" in blob
