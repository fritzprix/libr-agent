"""2565 O-3: runShell(asRoot=true) starting a background daemon either stops
the stray process with a notice (managed container) or refuses cleanly with the
managed-container guard (host/attached container), without hanging."""

from pathlib import Path

from verifier_lib import (
    load_evidence,
    load_trajectory,
    observation_result_texts,
    trajectory_steps,
)

RESULT_FILE = Path("/app/o3_result.txt")


def test_asroot_daemon_call_recorded_and_handled_cleanly():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "missing sessionId in E2E evidence"

    steps = trajectory_steps(load_trajectory())
    asroot_called = any(
        (call.get("arguments") or {}).get("asRoot") is True
        for step in steps
        for call in step.get("tool_calls") or []
    )
    assert asroot_called, "agent never invoked workspace__runShell with asRoot=true"

    obs = "\n".join(observation_result_texts(steps)).lower()
    # G1.5: `daemon_launched` is echoed by the daemon command itself, so it
    # proves the launch happened - exactly the failure O-3 tests for. Only
    # the product's stopped-process notice passes (the task declares managed
    # workspace mode, so the refusal path is not expected either).
    assert "stopped" in obs, (
        f"expected the stopped-process notice for the stray asRoot daemon "
        f"in observations: {obs[:400]!r}"
    )

    assert RESULT_FILE.is_file(), "o3_result.txt missing"
    assert RESULT_FILE.read_text(encoding="utf-8").strip() == "ASROOT_CHECKED"
