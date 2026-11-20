"""2565 P-8 & 2672 A-4: app restart while prompt waits closes tool call with restart message."""

import json
from pathlib import Path

from verifier_lib import load_evidence, require_session_evidence

RESTART_EVIDENCE = Path("/logs/agent/session_e2e_restart_evidence.json")
EXPECTED_TEXT = "User did not answer (app restarted)."


def _extract_tool_texts(messages: list[dict]) -> list[str]:
    texts: list[str] = []
    for msg in messages:
        if msg.get("role") != "tool":
            continue
        content = msg.get("content")
        if isinstance(content, str):
            texts.append(content)
        elif isinstance(content, list):
            for block in content:
                if isinstance(block, dict) and isinstance(block.get("text"), str):
                    texts.append(block["text"])
    return texts


def test_restart_while_prompt_waiting_closes_tool_call():
    require_session_evidence()
    evidence = load_evidence()
    interactions = evidence.get("resolvedInteractions") or []
    restart_triggers = [a for a in interactions if a.get("kind") == "restartOnPrompt"]
    assert restart_triggers, f"expected restartOnPrompt action, got {interactions!r}"
    assert restart_triggers[0].get("requestId"), "missing requestId on restartOnPrompt"

    assert RESTART_EVIDENCE.is_file(), "session_e2e_restart_evidence.json missing"
    restart = json.loads(RESTART_EVIDENCE.read_text(encoding="utf-8"))
    assert restart.get("restartExecuted") is True, f"instance restart failed: {restart!r}"

    by_name = {p["name"]: p for p in restart.get("http") or [] if isinstance(p, dict)}
    assert by_name.get("health", {}).get("status") == 200, f"post-restart /health failed: {by_name!r}"

    msg_probe = by_name.get("messages") or {}
    assert msg_probe.get("status") == 200, f"post-restart messages probe failed: {msg_probe!r}"
    payload = msg_probe.get("json") or {}
    messages = payload.get("messages") if isinstance(payload, dict) else []
    tool_texts = _extract_tool_texts(messages or [])
    assert any(EXPECTED_TEXT in text for text in tool_texts), (
        f"expected {EXPECTED_TEXT!r} in post-restart tool results, got {tool_texts!r}"
    )
