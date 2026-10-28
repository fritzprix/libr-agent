"""T04: the delivered message woke the session, and the woken session replied.

The harbor adapter delivers message m-5c1e after the session first goes idle
(task.toml [metadata.libragent_event]). A hook that replies by itself does not
count: the reply must be a tool call the session made after its wake message.
"""

import json
from pathlib import Path

STATE = Path("/app/.sms_monitor")
TRAJECTORY = Path("/logs/agent/trajectory.json")
MESSAGE_ID = "m-5c1e"
WAKE_PREFIX = "[wake callback]"


def _text(value) -> str:
    return value if isinstance(value, str) else json.dumps(value, ensure_ascii=False)


def test_message_was_delivered_to_a_subscription():
    record = STATE / "delivered" / f"{MESSAGE_ID}.json"
    assert record.exists(), "the message was not delivered: no sms_monitor subscription was registered"
    hooks = json.loads(record.read_text(encoding="utf-8"))["hooks"]
    assert any(hook["exit_code"] == 0 for hook in hooks), f"every hook failed: {hooks}"


def test_reply_was_recorded():
    replies = STATE / "replies.json"
    sent = json.loads(replies.read_text(encoding="utf-8")) if replies.exists() else []
    assert any(r["message_id"] == MESSAGE_ID and r["message"].strip() for r in sent), (
        f"no reply to {MESSAGE_ID}: {sent}"
    )


def test_woken_session_sent_the_reply():
    steps = json.loads(TRAJECTORY.read_text(encoding="utf-8"))["steps"]
    woken_at = next(
        (i for i, step in enumerate(steps) if _text(step.get("message", "")).lstrip().startswith(WAKE_PREFIX)),
        None,
    )
    assert woken_at is not None, "the session was never woken (no wake callback message)"
    replies = [
        call
        for step in steps[woken_at + 1 :]
        for call in step.get("tool_calls") or []
        if "reply" in _text(call.get("arguments")) and MESSAGE_ID in _text(call.get("arguments"))
    ]
    assert replies, f"the woken session made no reply call for {MESSAGE_ID}"
