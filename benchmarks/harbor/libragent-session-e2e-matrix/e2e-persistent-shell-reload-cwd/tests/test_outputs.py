"""2685 P-4: persistent shell keeps its working directory after reload."""

from pathlib import Path

from verifier_lib import (
    injected_values,
    load_evidence,
    load_trajectory,
    require_session_evidence,
)

CWD_FILE = Path("/app/p4_cwd.txt")


def test_persistent_shell_preserves_cwd_across_reload():
    require_session_evidence()
    evidence = load_evidence()
    answers = [
        a
        for a in evidence.get("resolvedInteractions") or []
        if a.get("kind") == "userPrompt" and a.get("delivered") is True
    ]
    assert answers, f"expected storeAs secret prompt to trigger shell reload, got {evidence!r}"

    assert CWD_FILE.is_file(), "p4_cwd.txt missing"
    recorded_cwd = CWD_FILE.read_text(encoding="utf-8").strip().replace("\\", "/")
    assert recorded_cwd.endswith("/sub_p4_dir"), (
        f"expected persistent shell cwd to stay in sub_p4_dir after reload, got {recorded_cwd!r}"
    )

    for secret in injected_values(evidence):
        assert secret not in str(load_trajectory()), "synthetic secret leaked into trajectory"
