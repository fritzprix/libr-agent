"""2672 M-3 + 2565 M-4:
- M-3: tool__registerServer approval preview masks literal query param values
  and URL fragments as '***' while keeping exact '{{secret:NAME}}' visible.
- M-4: cancelling the MCP secret prompt aborts registration and writes no
  server row."""

from pathlib import Path

from verifier_lib import (
    load_evidence,
    load_trajectory,
    observation_result_texts,
    trajectory_steps,
)

RESULT_FILE = Path("/app/mcp_cancel_result.txt")


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

    # Confirm the tool__list(external) observation does not list e2e-preview-cancel as registered.
    obs = observation_result_texts(trajectory_steps(load_trajectory()))
    assert obs, "expected tool observations in trajectory"
    assert not any(
        '"name": "e2e-preview-cancel"' in text or "e2e-preview-cancel (" in text
        for text in obs
    ), "e2e-preview-cancel appears to have been saved despite secret cancellation"
