"""T04: an event subscription that wakes the session must be registered."""

import json
from pathlib import Path

from critical_path import grade_trajectory

TRAJECTORY = Path("/logs/agent/trajectory.json")
STORE = Path("/app/.relay/subscriptions.json")


def test_subscription_registered():
    assert STORE.exists(), "no relayctl subscription was registered"
    subs = json.loads(STORE.read_text(encoding="utf-8"))
    matching = [s for s in subs if s.get("topic") == "release.gate.passed"]
    assert matching, f"no subscription on release.gate.passed: {subs}"
    assert any("LIBRAGENT_WAKE_URL" in s.get("exec", "") for s in matching), (
        "the subscription's --exec does not call $LIBRAGENT_WAKE_URL"
    )


def test_critical_path():
    """`__runShell` running `relayctl subscribe` (last call succeeds)"""
    failures = grade_trajectory("T04", TRAJECTORY)
    assert not failures, failures
