"""#2672 S-5: a builtin tool boolean argument sent as the string "true" is
parsed by the shared helper with a warning and the call still runs, and
ui__askUser no longer rejects a question by key-name rules (the question id
``options`` used to collide with a reserved key). Graded from harness
evidence, tool results, and workspace files — never assistant prose."""

from pathlib import Path

from verifier_lib import (
    load_evidence,
    load_trajectory,
    observation_result_texts,
    trajectory_steps,
)

BOOL_OK = Path("/app/bool_ok.txt")
ASK_ANSWER = Path("/app/ask_answer.txt")


def test_evidence_present():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "harness must write session_e2e_evidence.json"


def test_string_boolean_parsed_with_warning_and_call_ran():
    texts = observation_result_texts(trajectory_steps(load_trajectory()))
    warned = [
        text
        for text in texts
        if 'Warning: include_checked was the string "true"' in text
    ]
    assert warned, (
        "the shared args helper never emitted its string-boolean warning, so "
        "the tool call was not sent as a string (or the coercion was dropped)"
    )
    assert BOOL_OK.is_file(), (
        "the agent never confirmed the coerced call returned successfully "
        "(bool_ok.txt missing)"
    )
    assert BOOL_OK.read_text(encoding="utf-8").strip() == "COERCION_OK"


def test_askuser_accepted_reserved_sounding_question_key():
    evidence = load_evidence()
    answered = [
        action
        for action in evidence.get("resolvedInteractions") or []
        if action.get("kind") == "userPrompt" and action.get("action") == "answer"
    ]
    assert answered, (
        "no ui__askUser prompt reached the harness — the question with id "
        f"'options' was rejected. Actions: {evidence.get('resolvedInteractions')!r}"
    )
    assert answered[0].get("delivered"), (
        f"the askUser answer was never delivered: {answered[0]!r}"
    )
    assert ASK_ANSWER.is_file(), "the agent never wrote the delivered answer"
    assert ASK_ANSWER.read_text(encoding="utf-8").strip(), "ask_answer.txt is empty"
