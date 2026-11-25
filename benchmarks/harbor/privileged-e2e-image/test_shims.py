"""Unit tests for the privileged-path E2E test container shims (#2904)."""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

BIN_DIR = Path(__file__).resolve().parent / "bin"


def test_fake_pkexec_fail_mode_logs_invocation_and_exits_126(tmp_path: Path):
    env = {
        **os.environ,
        "LIBRAGENT_E2E_PRIVILEGED_LOG_DIR": str(tmp_path),
        "LIBRAGENT_E2E_FAKE_PKEXEC_MODE": "fail",
    }
    proc = subprocess.run(
        ["bash", str(BIN_DIR / "pkexec"), "id", "-u"],
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 126
    assert "e2e-fake-pkexec: refused" in proc.stderr

    log_lines = (tmp_path / "pkexec.log").read_text(encoding="utf-8").splitlines()
    assert len(log_lines) == 1
    entry = json.loads(log_lines[0])
    assert entry["tool"] == "pkexec"
    assert entry["mode"] == "fail"
    assert entry["argv"] == ["id", "-u"]


def test_fake_pkexec_succeed_mode_executes_command_and_logs(tmp_path: Path):
    env = {
        **os.environ,
        "LIBRAGENT_E2E_PRIVILEGED_LOG_DIR": str(tmp_path),
        "LIBRAGENT_E2E_FAKE_PKEXEC_MODE": "succeed",
    }
    proc = subprocess.run(
        ["bash", str(BIN_DIR / "pkexec"), "echo", "pkexec-ok"],
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0
    assert proc.stdout.strip() == "pkexec-ok"

    entry = json.loads((tmp_path / "pkexec.log").read_text(encoding="utf-8").strip())
    assert entry["mode"] == "succeed"
    assert entry["argv"] == ["echo", "pkexec-ok"]


def test_fake_askpass_succeed_and_fail_never_log_password(tmp_path: Path):
    secret = "synthetic-askpass-secret-2904"
    env_ok = {
        **os.environ,
        "LIBRAGENT_E2E_PRIVILEGED_LOG_DIR": str(tmp_path),
        "LIBRAGENT_E2E_FAKE_ASKPASS_MODE": "succeed",
        "LIBRAGENT_E2E_FAKE_ASKPASS_VALUE": secret,
    }
    ok = subprocess.run(
        ["bash", str(BIN_DIR / "e2e-askpass"), "[sudo] password for e2etest:"],
        env=env_ok,
        capture_output=True,
        text=True,
        check=False,
    )
    assert ok.returncode == 0
    assert ok.stdout.strip() == secret

    env_fail = {
        **os.environ,
        "LIBRAGENT_E2E_PRIVILEGED_LOG_DIR": str(tmp_path),
        "LIBRAGENT_E2E_FAKE_ASKPASS_MODE": "fail",
        "LIBRAGENT_E2E_FAKE_ASKPASS_VALUE": secret,
    }
    fail = subprocess.run(
        ["bash", str(BIN_DIR / "e2e-askpass"), "[sudo] password for e2etest:"],
        env=env_fail,
        capture_output=True,
        text=True,
        check=False,
    )
    assert fail.returncode == 1

    log_text = (tmp_path / "askpass.log").read_text(encoding="utf-8")
    assert secret not in log_text, "askpass log must never contain the password value"
    entries = [json.loads(line) for line in log_text.splitlines()]
    assert [e["mode"] for e in entries] == ["succeed", "fail"]


def test_nopasswd_probe_prints_uid():
    proc = subprocess.run(
        ["bash", str(BIN_DIR / "e2e-nopasswd-probe")],
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0
    assert proc.stdout.strip().startswith("NOPASSWD_PROBE_UID=")
