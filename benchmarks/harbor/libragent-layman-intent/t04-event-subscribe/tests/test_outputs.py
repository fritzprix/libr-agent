"""T04: a hook that wakes the session must be registered (graded statically, at idle)."""

import json
from pathlib import Path

from critical_path import calls_wake, grade_trajectory

TRAJECTORY = Path("/logs/agent/trajectory.json")
STORE = Path("/app/.sms_monitor/subscriptions.json")


def test_subscription_registered():
    assert STORE.exists(), "no sms_monitor subscription was registered"
    subs = json.loads(STORE.read_text(encoding="utf-8"))
    assert subs, "no sms_monitor subscription was registered"
    assert any(calls_wake(s.get("hook", "")) for s in subs), (
        "no subscription's hook makes an HTTP request to the session wake URL"
    )


def test_critical_path():
    """`__runShell` running `sms_monitor subscribe` (last call succeeds); no waitForProcess or polling"""
    failures = grade_trajectory("T04", TRAJECTORY)
    assert not failures, failures
