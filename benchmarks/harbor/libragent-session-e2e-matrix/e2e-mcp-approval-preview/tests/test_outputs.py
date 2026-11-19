"""2672 M-3 + 2565 M-4:
- M-3: tool__registerServer approval preview masks literal query param values
  and URL fragments as '***' while keeping exact '{{secret:NAME}}' visible.
- M-4: cancelling the MCP secret prompt aborts registration and writes no
  server row."""

import re
from pathlib import Path

from verifier_lib import (
    load_evidence,
    load_trajectory,
    trajectory_steps,
)

RESULT_FILE = Path("/app/mcp_cancel_result.txt")
# USER_PROMPT_CANCELLED_MESSAGE in src-tauri/src/agent/state/user_prompt.rs.
CANCELLED_MESSAGE = "User cancelled the prompt."
LIST_CALL = re.compile(r"tool__listServers|tool__list\b")
REGISTER_CALL = re.compile(r"tool__registerServer")


def _call_scoped_observations(steps: list[dict], pattern: re.Pattern) -> list[str]:
    """Text of tool results produced by calls whose function name matches.

    The ATIF export links results to calls via ``observation.results[i].source_call_id``;
    when that id is absent the result belongs to the same step's calls.
    """
    texts: list[str] = []
    for step in steps:
        call_ids = {
            call.get("tool_call_id")
            for call in step.get("tool_calls") or []
            if pattern.search(str(call.get("function_name") or ""))
        }
        if not call_ids:
            continue
        for result in (step.get("observation") or {}).get("results") or []:
            content = result.get("content")
            if not isinstance(content, str):
                continue
            source = result.get("source_call_id")
            if source is None or source in call_ids:
                texts.append(content)
    return texts


def _call_count(steps: list[dict], pattern: re.Pattern) -> int:
    return sum(
        1
        for step in steps
        for call in step.get("tool_calls") or []
        if pattern.search(str(call.get("function_name") or ""))
    )


def test_approval_preview_masks_literal_query_and_fragment_keeps_template():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "missing sessionId in E2E evidence"
    approvals = [
        a
        for a in evidence.get("resolvedInteractions") or []
        if a.get("kind") == "toolApproval"
        and "registerServer" in str(a.get("toolName") or "")
    ]
    assert approvals, (
        "expected a toolApproval record for tool__registerServer; "
        f"resolvedInteractions={evidence.get('resolvedInteractions')!r}"
    )
    preview = str(approvals[0].get("commandPreview") or "")
    assert preview, f"missing commandPreview in approval record: {approvals[0]!r}"
    assert "plainSecret123" not in preview, (
        f"literal query param leaked unmasked in approval preview: {preview!r}"
    )
    assert "fragSecret456" not in preview, (
        f"URL fragment leaked unmasked in approval preview: {preview!r}"
    )
    assert "***" in preview, f"expected *** masking in approval preview: {preview!r}"
    assert "{{secret:E2E_PREVIEW_KEY}}" in preview, (
        f"exact secret template must stay visible in approval preview: {preview!r}"
    )


def test_secret_prompt_cancelled_and_no_server_saved():
    evidence = load_evidence()
    cancels = [
        a
        for a in evidence.get("resolvedInteractions") or []
        if a.get("kind") == "userPrompt"
        and a.get("action") == "cancel"
        and a.get("delivered") is True
    ]
    assert cancels, (
        "expected a delivered userPrompt cancel record for E2E_PREVIEW_KEY; "
        f"resolvedInteractions={evidence.get('resolvedInteractions')!r}"
    )

    assert RESULT_FILE.is_file(), "mcp_cancel_result.txt missing"
    assert RESULT_FILE.read_text(encoding="utf-8").strip() == "NOT_REGISTERED"

    steps = trajectory_steps(load_trajectory())

    # (a) The product itself must report the cancellation: the
    # tool__registerServer observation carries the cancel error, so the
    # verifier does not rely on any agent-written file.
    assert _call_count(steps, REGISTER_CALL) >= 1, (
        "expected a tool__registerServer call in the trajectory"
    )
    register_obs = _call_scoped_observations(steps, REGISTER_CALL)
    assert register_obs, "no observation recorded for tool__registerServer"
    assert any(CANCELLED_MESSAGE in text for text in register_obs), (
        "the tool__registerServer observation must carry the cancellation error "
        f"({CANCELLED_MESSAGE!r}); got: {register_obs!r}"
    )

    # (b) A tool__listServers(scope=external) call must have happened, and its
    # real output (markdown rows '| External: <name> |' and the '• <name> →'
    # id list) must not mention the cancelled server.
    assert _call_count(steps, LIST_CALL) >= 1, (
        "expected a tool__listServers call in the trajectory; the agent skipped "
        "the registration-list verification"
    )
    list_obs = _call_scoped_observations(steps, LIST_CALL)
    assert list_obs, "no observation recorded for tool__listServers"
    for text in list_obs:
        assert "e2e-preview-cancel" not in text, (
            "e2e-preview-cancel appears in the tool__listServers output despite "
            f"secret cancellation: {text!r}"
        )
