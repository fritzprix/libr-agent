"""#2672 K-3: the mcp-installer flow registers a server that needs a key by
writing the secret as a ``{{secret:NAME}}`` template, and the agent never
asks for the secret value in chat. Graded from harness evidence, the full
trajectory (tool-call arguments included), and workspace files — never
assistant prose."""

import json
import os
from pathlib import Path

from verifier_lib import load_evidence, load_trajectory

TEMPLATE = "{{secret:E2E_WEATHER_KEY}}"
# The value the harness would inject if a prompt were answered (env var when
# set, else the automation fallback — see session_e2e/automation.py). A
# correctly behaved run never triggers a prompt here, so it must not appear
# anywhere.
SECRET = os.environ.get("LIBRAGENT_E2E_SYNTHETIC_SECRET") or "e2e-synthetic-answer"
REGISTERED = Path("/app/mcp_registered.txt")


def test_evidence_present():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "harness must write session_e2e_evidence.json"


def test_secret_stored_as_template():
    trajectory_blob = json.dumps(load_trajectory())
    assert TEMPLATE in trajectory_blob, (
        "the registration never used the {{secret:NAME}} template for "
        "E2E_WEATHER_KEY"
    )


def test_agent_never_asked_for_the_secret_value():
    evidence = load_evidence()
    prompts = [
        action
        for action in evidence.get("resolvedInteractions") or []
        if action.get("kind") == "userPrompt"
    ]
    assert not prompts, (
        "the agent asked the user for input during registration — the flow "
        f"must reference the template only. Actions: {prompts!r}"
    )
    trajectory_blob = json.dumps(load_trajectory())
    assert SECRET not in trajectory_blob, (
        "the synthetic secret leaked into the trajectory; no real or "
        "synthetic key value should ever reach this session"
    )


def test_registration_completed():
    assert REGISTERED.is_file(), "the agent never wrote mcp_registered.txt"
    assert REGISTERED.read_text(encoding="utf-8").strip() == "REGISTERED"
