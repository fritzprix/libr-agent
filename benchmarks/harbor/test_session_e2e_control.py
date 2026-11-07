"""Contract tests for the Session E2E control-plane client helpers.

Guards the gatekeeper findings:
- the capability token is read from the trial instance's data dir only —
  no env-var token and no fallback to a production profile directory;
- the verifier's stale-execution negative control actually discriminates
  (fails on empty evidence, passes on real evidence).
"""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path

import pytest

from benchmarks.harbor.session_e2e.control import (
    E2E_HEADER,
    e2e_control_configured,
    read_capability_token,
)


def test_header_name_matches_backend():
    assert E2E_HEADER == "x-libragent-e2e-control"


def test_e2e_control_not_configured_without_opt_in(tmp_path, monkeypatch):
    monkeypatch.delenv("LIBRAGENT_E2E_CONTROL", raising=False)
    (tmp_path / "e2e_control.cap").write_text("abc123", encoding="utf-8")
    assert not e2e_control_configured(tmp_path)


def test_e2e_control_not_configured_without_token_file(tmp_path, monkeypatch):
    monkeypatch.setenv("LIBRAGENT_E2E_CONTROL", "1")
    assert not e2e_control_configured(tmp_path)


def test_e2e_control_configured_with_opt_in_and_token(tmp_path, monkeypatch):
    monkeypatch.setenv("LIBRAGENT_E2E_CONTROL", "1")
    (tmp_path / "e2e_control.cap").write_text("abc123", encoding="utf-8")
    assert e2e_control_configured(tmp_path)


def test_read_capability_token_missing_file(tmp_path):
    assert read_capability_token(tmp_path) is None


def test_read_capability_token_from_data_dir(tmp_path):
    (tmp_path / "e2e_control.cap").write_text("  abc123\n", encoding="utf-8")
    assert read_capability_token(tmp_path) == "abc123"


def test_read_capability_token_ignores_production_home(tmp_path, monkeypatch):
    """Only the instance data dir is consulted — never a home fallback.

    Point HOME/USERPROFILE at a tree that contains a cap file and assert the
    reader still returns None for an unrelated instance data dir.
    """
    fake_home = tmp_path / "home"
    prod = fake_home / "AppData" / "Local" / "com.fritzprix.libragent"
    prod.mkdir(parents=True)
    (prod / "e2e_control.cap").write_text("prod-token", encoding="utf-8")
    monkeypatch.setenv("HOME", str(fake_home))
    monkeypatch.setenv("USERPROFILE", str(fake_home))
    instance_dir = tmp_path / "instance"
    instance_dir.mkdir()
    assert read_capability_token(instance_dir) is None


def _load_verifier_lib():
    lib_path = (
        Path(__file__).resolve().parent
        / "libragent-session-e2e-matrix/shared/verifier_lib.py"
    )
    spec = importlib.util.spec_from_file_location("verifier_lib", lib_path)
    assert spec and spec.loader
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def test_negative_stale_execution_rejected_by_contract(tmp_path):
    """Empty evidence must fail the interactive-prompt contract."""
    mod = _load_verifier_lib()
    mod.EVIDENCE = tmp_path / "missing-evidence.json"
    with pytest.raises(AssertionError, match="sessionId"):
        mod.negative_control_stale_execution_must_fail()


def test_real_execution_satisfies_contract(tmp_path):
    """The same call must pass when evidence carries a real sessionId.

    Paired with the negative test above so the negative control is not
    vacuous — it discriminates between stale and real executions.
    """
    mod = _load_verifier_lib()
    evidence = tmp_path / "evidence.json"
    evidence.write_text(json.dumps({"sessionId": "sess-real-123"}), encoding="utf-8")
    mod.EVIDENCE = evidence
    mod.negative_control_stale_execution_must_fail()
