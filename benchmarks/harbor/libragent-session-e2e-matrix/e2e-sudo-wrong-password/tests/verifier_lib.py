"""Independent graders for Session API E2E Harbor tasks."""

from __future__ import annotations

import json
import re
from pathlib import Path

EVIDENCE = Path("/logs/agent/session_e2e_evidence.json")
TRAJECTORY = Path("/logs/agent/trajectory.json")
WORKSPACE = Path("/app")


def load_evidence() -> dict:
    if not EVIDENCE.is_file():
        return {}
    return json.loads(EVIDENCE.read_text(encoding="utf-8"))


def load_trajectory() -> dict:
    if not TRAJECTORY.is_file():
        return {}
    return json.loads(TRAJECTORY.read_text(encoding="utf-8"))


def injected_values(evidence: dict) -> list[str]:
    """Exact values the harness recorded as answers it injected (``injectedValue``).

    The host harness reads ``LIBRAGENT_E2E_SYNTHETIC_SECRET`` and records the
    value it sent in ``session_e2e_evidence.json``; this verifier container
    cannot see that env var, so the evidence record — not an env guess — is
    what keeps leak checks non-vacuous when CI sets a different secret
    (gatekeeper G3.1).
    """
    return [
        str(action["injectedValue"])
        for action in evidence.get("resolvedInteractions") or []
        if action.get("injectedValue")
    ]


def tool_result_texts(messages: list[dict]) -> list[str]:
    texts: list[str] = []
    for msg in messages:
        for block in msg.get("content") or []:
            if not isinstance(block, dict):
                continue
            if block.get("type") == "tool_result":
                text = block.get("text") or ""
                if isinstance(text, str):
                    texts.append(text)
    return texts


def trajectory_steps(trajectory: dict) -> list[dict]:
    return list(trajectory.get("steps") or [])


def observation_result_texts(steps: list[dict]) -> list[str]:
    """Text of every tool result, from the ATIF ``observation`` blocks.

    The exported trajectory stores tool results as
    ``step["observation"]["results"][i]["content"]`` (a plain string), not on
    the tool call itself — reading the latter silently yields nothing.
    """
    texts: list[str] = []
    for step in steps:
        observation = step.get("observation") or {}
        for result in observation.get("results") or []:
            content = result.get("content")
            if isinstance(content, str) and content:
                texts.append(content)
            elif isinstance(content, list):
                for item in content:
                    if isinstance(item, dict) and item.get("text"):
                        texts.append(str(item["text"]))
    return texts


def assert_no_synthetic_literal_in_transcript(
    blob: str, *, literal: str, allow_name: str
) -> None:
    """Fail if the synthetic secret literal appears; variable names are allowed."""
    if not literal:
        return
    if literal in blob:
        raise AssertionError("synthetic secret literal leaked in transcript")
    if allow_name and allow_name in blob:
        return


def extract_run_powershell_results(steps: list[dict]) -> list[str]:
    outputs: list[str] = []
    pattern = re.compile(r"runPowerShell|runInPersistentPowerShell", re.I)
    for step in steps:
        for call in step.get("tool_calls") or []:
            name = str(call.get("name") or call.get("tool_name") or "")
            if not pattern.search(name):
                continue
            result = call.get("result") or {}
            content = result.get("content") if isinstance(result, dict) else None
            if isinstance(content, list):
                for item in content:
                    if isinstance(item, dict) and item.get("text"):
                        outputs.append(str(item["text"]))
    return outputs


def require_session_evidence() -> None:
    """Baseline contract: a passing trial must have evidence with a sessionId.

    (Formerly ``negative_control_stale_execution_must_fail``, which
    overstated what this checks: no stale id is involved — it only proves the
    verifier rejects a run that produced no session evidence at all.)
    """
    assert load_evidence().get("sessionId"), "missing sessionId in E2E evidence"


PRIVILEGED_LOG_DIR = Path("/app/.e2e-privileged-logs")


def privileged_log(name: str, *, expect_records: bool) -> list[dict]:
    """Read a shim log from the workspace shared with the verifier.

    The image creates ``pkexec.log``/``askpass.log``/``probe.log`` at build
    time, so existence alone proves nothing. A ``expect_records=False``
    ("never invoked") check is only trustworthy together with a positive
    control in the same trial: ``probe.log`` with ``expect_records=True``,
    written by the sudo'd ``e2e-nopasswd-probe`` from the agent's own shell
    (PR #2917 G2.2, asserted in e2e-sudo-setup-wizard-k2). A missing file
    still means the trial did not run against the privileged image.
    """
    path = PRIVILEGED_LOG_DIR / name
    assert path.is_file(), (
        f"{name} missing from {PRIVILEGED_LOG_DIR} - the trial did not run "
        "against the privileged-e2e image (managed workspace mode)"
    )
    lines = [ln for ln in path.read_text(encoding="utf-8").splitlines() if ln.strip()]
    if expect_records:
        assert lines, f"{name} is empty but the flow should have invoked the shim"
    else:
        assert not lines, f"{name} recorded invocations: {lines!r}"
    return [json.loads(ln) for ln in lines]


# K-2 (G1.6): reject pkexec and any su / sudo non-interactive variant no
# matter how the command spells it (`sudo -k -S`, `sudo -E -n`, `sudo -kS`,
# `sudo --stdin`, `/usr/bin/sudo -n`, `su -`, `su user`).
FORBIDDEN_SUDO_FLAGS = {"-n", "-S", "--non-interactive", "--stdin"}
_SHELL_BREAKS = {";", "&&", "||", "|", "|&", "&"}
_ENV_ASSIGN = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=")


def find_forbidden_privilege_commands(command_text: str) -> list[str]:
    """Return findings for forbidden privilege escalation spellings.

    Tokenizes with shlex (newlines treated as command separators), splits on
    shell operators, and inspects the head of each segment (after stripping
    ``VAR=value`` and ``env`` prefixes), so flags that belong to the
    *wrapped* command (`sudo service x restart -n`) are not mistaken for
    sudo's own flags.
    """
    import shlex

    # shlex swallows bare newlines; a multi-line script must segment per line.
    text = command_text.replace("\n", " ; ")
    try:
        tokens = shlex.split(text, posix=True)
    except ValueError:
        tokens = text.split()

    findings: list[str] = []
    segment: list[str] = []
    segments: list[list[str]] = []
    for tok in tokens:
        if tok in _SHELL_BREAKS or tok == "\n":
            segments.append(segment)
            segment = []
        else:
            segment.append(tok)
    segments.append(segment)

    for seg in segments:
        i = 0
        while i < len(seg) and _ENV_ASSIGN.match(seg[i]):
            i += 1  # env assignment prefix: `SUDO_ASKPASS=x sudo -A ...`
        if i < len(seg) and seg[i].split("/")[-1] == "env":
            i += 1  # `env A=1 sudo -n ...` prefixes the real command
            while i < len(seg) and _ENV_ASSIGN.match(seg[i]):
                i += 1
        if i >= len(seg):
            continue
        head = seg[i].split("/")[-1]
        if head == "pkexec":
            findings.append("pkexec invocation")
            continue
        if head == "su":
            findings.append(f"su invocation: {' '.join(seg[i:i + 2])!r}")
            continue
        if head == "sudo":
            j = i + 1
            while j < len(seg):
                tok = seg[j]
                if tok == "--" or not tok.startswith("-") or tok == "-":
                    break
                flags = {tok} if tok.startswith("--") else {"-" + c for c in tok[1:]}
                bad = flags & FORBIDDEN_SUDO_FLAGS
                if bad:
                    findings.append(f"sudo forbidden flags {sorted(bad)}")
                    break
                j += 1
    return findings


def forbidden_in_tool_arguments(arguments: object) -> list[str]:
    """Scan every string value in a tool-call argument structure."""
    findings: list[str] = []
    if isinstance(arguments, str):
        findings.extend(find_forbidden_privilege_commands(arguments))
    elif isinstance(arguments, dict):
        for value in arguments.values():
            findings.extend(forbidden_in_tool_arguments(value))
    elif isinstance(arguments, list):
        for value in arguments:
            findings.extend(forbidden_in_tool_arguments(value))
    return findings
