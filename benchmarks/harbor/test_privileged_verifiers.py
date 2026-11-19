"""Repo-side tests for the privileged E2E verifiers (#2904, gatekeeper round 1).

Covers G1.6 (the K-2 forbidden-command tokenizer must reject every spelling)
and S1.2 (the four task copies of the image files and verifier_lib.py must
stay byte-identical to the canonical copy)."""

from __future__ import annotations

import hashlib
import importlib.util
from pathlib import Path

import pytest

HARBOR = Path(__file__).parent
MATRIX = HARBOR / "libragent-session-e2e-matrix"
CANONICAL_IMAGE = HARBOR / "privileged-e2e-image"
PRIVILEGED_TASKS = (
    "e2e-sudo-askpass-flow",
    "e2e-sudo-setup-wizard-k2",
    "e2e-sudo-wrong-password",
    "e2e-sudo-cancel-x1",
)


def _load_verifier_lib(task: str):
    path = MATRIX / task / "tests" / "verifier_lib.py"
    spec = importlib.util.spec_from_file_location(f"verifier_lib_{task}", path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


VERIFIER_LIB = _load_verifier_lib("e2e-sudo-setup-wizard-k2")


@pytest.mark.parametrize(
    "command",
    [
        "sudo -n apt update",
        "sudo -S apt update",
        "sudo -k -S apt update",
        "sudo -E -n apt update",
        "sudo --stdin apt update",
        "sudo --non-interactive apt update",
        "sudo -kS apt update",
        "sudo -nS apt update",
        "/usr/bin/sudo -n apt update",
        "env A=1 sudo --non-interactive apt update",
        "SUDO_ASKPASS=/bin/false sudo -n true",
        "systemctl status && sudo -n reboot",
        "su -",
        "su e2etest",
        "/bin/su root -c id",
        "pkexec apt update",
        "/usr/local/bin/pkexec id",
        "cd /app && pkexec sh",
    ],
)
def test_forbidden_spellings_are_rejected(command: str) -> None:
    findings = VERIFIER_LIB.find_forbidden_privilege_commands(command)
    assert findings, f"must reject: {command}"


@pytest.mark.parametrize(
    "command",
    [
        "sudo id",
        "sudo systemctl restart docker",
        "sudo service nginx restart -n",  # -n belongs to the wrapped command
        "echo su file",
        "grep -n su users.txt",
        "echo 'sudo -n' in docs",
        "sudo -k drop caches",  # -k alone is fine (credential cache reset)
        "apt install -y result",
        "result",
    ],
)
def test_allowed_commands_are_not_flagged(command: str) -> None:
    findings = VERIFIER_LIB.find_forbidden_privilege_commands(command)
    assert not findings, f"must allow: {command} (got {findings})"


def test_forbidden_in_tool_arguments_scans_nested_values() -> None:
    arguments = {
        "command": "make all",
        "script": "cd /app\npkexec sh\n",
        "cwd": "/app",
    }
    findings = VERIFIER_LIB.forbidden_in_tool_arguments(arguments)
    assert any("pkexec" in f for f in findings)


def _sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


@pytest.mark.parametrize("task", PRIVILEGED_TASKS)
def test_task_image_copies_match_canonical(task: str) -> None:
    env = MATRIX / task / "environment"
    assert _sha(env / "Dockerfile") == _sha(CANONICAL_IMAGE / "Dockerfile"), task
    for name in ("pkexec", "e2e-askpass", "e2e-nopasswd-probe"):
        assert _sha(env / "bin" / name) == _sha(
            CANONICAL_IMAGE / "bin" / name
        ), f"{task}/{name}"


@pytest.mark.parametrize("task", PRIVILEGED_TASKS)
def test_verifier_lib_copies_match_canonical(task: str) -> None:
    canonical = MATRIX / PRIVILEGED_TASKS[0] / "tests" / "verifier_lib.py"
    other = MATRIX / task / "tests" / "verifier_lib.py"
    assert _sha(canonical) == _sha(other), task


def test_privileged_tasks_declare_managed_mode() -> None:
    for task in PRIVILEGED_TASKS + ("e2e-asroot-daemon-o3",):
        text = (MATRIX / task / "task.toml").read_text(encoding="utf-8")
        assert 'requires_workspace_mode = "managed"' in text, task


def test_keychain_task_declares_fail_write_mode() -> None:
    text = (MATRIX / "e2e-keychain-fail-and-fallback" / "task.toml").read_text(
        encoding="utf-8"
    )
    assert 'mock_keychain_mode = "fail-write"' in text


def test_spec_parsing_reads_new_fields(tmp_path: Path) -> None:
    task_dir = tmp_path / "task"
    task_dir.mkdir()
    (task_dir / "task.toml").write_text(
        """
[metadata.libragent_session_e2e]
matrix_id = "X-9"
requires_workspace_mode = "Managed"
mock_keychain_mode = "fail-write"
""",
        encoding="utf-8",
    )
    logs_dir = tmp_path / "agent"
    logs_dir.mkdir()
    (tmp_path / "config.json").write_text(
        f'{{"task": {{"path": "{task_dir}"}}}}', encoding="utf-8"
    )

    from benchmarks.harbor.session_e2e.automation import load_session_e2e_spec

    spec = load_session_e2e_spec(logs_dir)
    assert spec is not None
    assert spec.requires_workspace_mode == "managed"
    assert spec.mock_keychain_mode == "fail-write"


def test_spec_parsing_rejects_bad_mock_mode(tmp_path: Path) -> None:
    task_dir = tmp_path / "task"
    task_dir.mkdir()
    (task_dir / "task.toml").write_text(
        """
[metadata.libragent_session_e2e]
matrix_id = "X-9"
mock_keychain_mode = "chaos"
""",
        encoding="utf-8",
    )
    logs_dir = tmp_path / "agent"
    logs_dir.mkdir()
    (tmp_path / "config.json").write_text(
        f'{{"task": {{"path": "{task_dir}"}}}}', encoding="utf-8"
    )

    from benchmarks.harbor.session_e2e.automation import load_session_e2e_spec

    with pytest.raises(RuntimeError, match="mock_keychain_mode"):
        load_session_e2e_spec(logs_dir)
