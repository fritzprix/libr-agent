"""2565 A-1/A-2 (wire contract): a multi-question ``ui__askUser`` call with
options and multiSelect must come back as a JSON object keyed by question id,
with the multiSelect answer as a list of values. The harness answers the
dialog with the task's ``answer_value`` through the control plane. Graded
from the delivered tool result and the workspace file — never assistant
prose. Radio/checkbox rendering itself remains ui-only."""

import json
from pathlib import Path

from verifier_lib import load_evidence, load_trajectory, observation_result_texts, trajectory_steps

ASK_RESULT = Path("/app/ask_result.json")


def test_evidence_present_and_delivered():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "harness must write session_e2e_evidence.json"
    answered = [
        action
        for action in evidence.get("resolvedInteractions") or []
        if action.get("kind") == "userPrompt" and action.get("action") == "answer"
    ]
    assert answered and answered[0].get("delivered"), (
        f"the harness answer was never delivered: {evidence.get('resolvedInteractions')!r}"
    )


def test_result_is_json_keyed_by_question_id():
    assert ASK_RESULT.is_file(), "the agent never wrote ask_result.json"
    result = json.loads(ASK_RESULT.read_text(encoding="utf-8"))
    assert set(result) == {"city", "toppings", "note"}, (
        f"result must be keyed by the three question ids, got {sorted(result)}"
    )
    for question_id, entry in result.items():
        assert isinstance(entry, dict) and "question" in entry and "answer" in entry, (
            f"'{question_id}' must carry question + answer, got {entry!r}"
        )


def test_options_and_multiselect_shapes():
    result = json.loads(ASK_RESULT.read_text(encoding="utf-8"))
    assert result["city"]["answer"] == "Seoul", (
        "an options question must answer with the chosen option string"
    )
    assert result["toppings"]["answer"] == ["Cheese", "Onion"], (
        "a multiSelect question must answer with a JSON list of values"
    )
    assert result["note"]["answer"] == "e2e-note"


def test_result_reached_the_agent_verbatim():
    texts = observation_result_texts(trajectory_steps(load_trajectory()))
    written = json.loads(ASK_RESULT.read_text(encoding="utf-8"))
    parsed_results = []
    for text in texts:
        try:
            parsed_results.append(json.loads(text))
        except ValueError:
            continue
    assert written in parsed_results, (
        "ask_result.json does not match any delivered tool result — the "
        "keyed JSON answer must reach the agent and be copied verbatim"
    )
