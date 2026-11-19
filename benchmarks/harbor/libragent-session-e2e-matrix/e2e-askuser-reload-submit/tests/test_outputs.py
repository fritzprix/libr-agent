"""2565 P-5 & X-5: pending prompt survives session reload/re-poll and delivers JSON answer."""

from pathlib import Path

from verifier_lib import (
    load_evidence,
    load_trajectory,
    observation_result_texts,
    require_session_evidence,
    trajectory_steps,
)

RESULT_FILE = Path("/app/reload_answer.txt")


def test_prompt_survived_repoll_and_delivered_json_answer():
    require_session_evidence()
    evidence = load_evidence()
    interactions = evidence.get("resolvedInteractions") or []

    observed = [a for a in interactions if a.get("kind") == "userPromptObserved"]
    assert observed, f"expected userPromptObserved before answering, got {interactions!r}"
    assert any(a.get("sessionRefetched") is True for a in observed), (
        f"expected session metadata refetch during waiting prompt, got {observed!r}"
    )

    answered = [
        a
        for a in interactions
        if a.get("kind") == "userPrompt"
        and a.get("action") == "answer"
        and a.get("delivered") is True
        and a.get("survivedRepoll") is True
    ]
    assert answered, f"expected delivered answer with survivedRepoll=true, got {interactions!r}"
    observed_ids = {a.get("requestId") for a in observed}
    assert any(a.get("requestId") in observed_ids for a in answered), (
        "answered requestId did not match the requestId observed before reload"
    )

    obs = "\n".join(observation_result_texts(trajectory_steps(load_trajectory())))
    assert "alpha" in obs and "after-reload" in obs, (
        f"expected JSON answer values in tool result observation, got: {obs[:400]!r}"
    )

    assert RESULT_FILE.is_file(), "reload_answer.txt missing — turn did not continue after answer"
    text = RESULT_FILE.read_text(encoding="utf-8")
    assert "alpha" in text and "after-reload" in text, f"unexpected result file: {text!r}"
