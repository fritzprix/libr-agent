from __future__ import annotations

import json
from pathlib import Path

import pytest

from benchmarks.harbor.session_e2e.control import (
    E2E_HEADER,
    e2e_control_configured,
    read_capability_token,
)


def test_e2e_control_not_configured_without_opt_in(monkeypatch):
    monkeypatch.delenv("LIBRAGENT_E2E_CONTROL", raising=False)
    assert not e2e_control_configured()


def test_read_capability_token_missing_without_env(tmp_path, monkeypatch):
    monkeypatch.delenv("LIBRAGENT_E2E_CONTROL_TOKEN", raising=False)
    monkeypatch.setenv("LIBRAGENT_DATA_DIR", str(tmp_path))
    assert read_capability_token() is None


def test_read_capability_token_from_data_dir(tmp_path, monkeypatch):
    monkeypatch.delenv("LIBRAGENT_E2E_CONTROL_TOKEN", raising=False)
    monkeypatch.setenv("LIBRAGENT_DATA_DIR", str(tmp_path))
    (tmp_path / "e2e_control.cap").write_text("abc123", encoding="utf-8")
    assert read_capability_token() == "abc123"


def test_negative_stale_execution_rejected_by_contract():
    """Verifier negative control: empty evidence must not imply pass."""
    import importlib.util

    lib_path = (
        Path(__file__).resolve().parent
        / "libragent-session-e2e-matrix/shared/verifier_lib.py"
    )
    spec = importlib.util.spec_from_file_location("verifier_lib", lib_path)
    assert spec and spec.loader
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    mod.EVIDENCE = Path("/nonexistent/evidence.json")
    with pytest.raises(AssertionError):
        mod.negative_control_stale_execution_must_fail()
