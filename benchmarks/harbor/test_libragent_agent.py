from __future__ import annotations

import pytest

from benchmarks.harbor.libragent_agent import (
    DEFAULT_EXECUTION_MODE,
    is_workflow_complete,
    resolve_execution_mode,
    resolve_poll_timeout_sec,
)


def test_resolve_execution_mode_defaults_to_unsafe() -> None:
    assert resolve_execution_mode(None, env={}) == DEFAULT_EXECUTION_MODE
    assert DEFAULT_EXECUTION_MODE == "unsafe"


def test_resolve_execution_mode_uses_explicit_value() -> None:
    assert resolve_execution_mode("yolo", env={"LIBRAGENT_EXECUTION_MODE": "unsafe"}) == "yolo"


def test_resolve_execution_mode_reads_env_when_explicit_missing() -> None:
    assert resolve_execution_mode(None, env={"LIBRAGENT_EXECUTION_MODE": "normal"}) == "normal"


def test_resolve_execution_mode_normalizes_case_and_whitespace() -> None:
    assert resolve_execution_mode("  UNSAFE  ", env={}) == "unsafe"


def test_resolve_execution_mode_rejects_invalid_values() -> None:
    with pytest.raises(ValueError, match="Invalid execution mode"):
        resolve_execution_mode("turbo", env={})


def test_workflow_complete_ignores_brief_idle_before_busy() -> None:
    assert is_workflow_complete("idle", seen_non_idle=False) is False
    assert is_workflow_complete("Idle", seen_non_idle=True) is True


def test_workflow_complete_treats_error_as_terminal() -> None:
    assert is_workflow_complete("error", seen_non_idle=False) is True


def test_workflow_complete_does_not_treat_paused_or_busy_as_done() -> None:
    assert is_workflow_complete("paused", seen_non_idle=True) is False
    assert is_workflow_complete("busy", seen_non_idle=True) is False
    assert is_workflow_complete("queued", seen_non_idle=True) is False


def test_resolve_poll_timeout_sec_from_env() -> None:
    assert resolve_poll_timeout_sec(None, env={}) is None
    assert resolve_poll_timeout_sec(None, env={"LIBRAGENT_POLL_TIMEOUT_SEC": "900"}) == 900.0
    assert resolve_poll_timeout_sec(120.0, env={"LIBRAGENT_POLL_TIMEOUT_SEC": "900"}) == 120.0
