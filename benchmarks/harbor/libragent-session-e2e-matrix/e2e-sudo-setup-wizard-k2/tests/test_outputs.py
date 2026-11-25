"""2672 K-2: setup-wizard root step asks once for SUDO_PASSWORD via storeAs,
uses plain sudo, and never invokes sudo -n, sudo -S, su, or pkexec.

The forbidden-command scan tokenizes each tool command (G1.6): `sudo -k -S`,
`sudo -E -n`, combined short flags like `sudo -kS`, `sudo --stdin`,
`/usr/bin/sudo -n`, and any `su` invocation are all rejected, while flags
that belong to the wrapped command (`sudo service x restart -n`) are not."""

import json
from pathlib import Path

from verifier_lib import (
    forbidden_in_tool_arguments,
    load_evidence,
    load_trajectory,
    privileged_log,
)

UID_FILE = Path("/app/root_uid.txt")


def test_asked_once_and_plain_sudo_succeeded():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "missing sessionId in E2E evidence"
    answers = [
        a
        for a in evidence.get("resolvedInteractions") or []
        if a.get("kind") == "userPrompt"
        and a.get("action") == "answer"
        and a.get("delivered") is True
    ]
    assert len(answers) == 1, f"expected exactly 1 SUDO_PASSWORD prompt, got {answers!r}"

    assert UID_FILE.is_file(), "root_uid.txt missing"
    assert UID_FILE.read_text(encoding="utf-8").strip() == "0"


def test_never_invoked_pkexec_su_or_forbidden_sudo_flags():
    # Positive control (G2.2) BEFORE the empty checks: the instruction's
    # sudo'd e2e-nopasswd-probe appends a record to probe.log through the
    # same shim path, so a non-empty probe.log proves this verifier sees
    # what the agent's privileged shell wrote. Only then is an empty
    # pkexec.log/askpass.log evidence of "never invoked" rather than of an
    # unreachable shared dir.
    probe_records = privileged_log("probe.log", expect_records=True)
    assert all(r.get("tool") == "e2e-nopasswd-probe" for r in probe_records)
    privileged_log("pkexec.log", expect_records=False)
    privileged_log("askpass.log", expect_records=False)

    trajectory = load_trajectory()
    for step in trajectory.get("steps") or []:
        for call in step.get("tool_calls") or []:
            findings = forbidden_in_tool_arguments(call.get("arguments") or {})
            assert not findings, (
                f"forbidden privilege command in tool_call arguments: "
                f"{findings!r} ({json.dumps(call.get('arguments') or {})!r})"
            )
