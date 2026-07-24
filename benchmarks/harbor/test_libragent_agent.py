from __future__ import annotations

import pytest

from benchmarks.harbor.libragent_agent import (
    DEFAULT_EXECUTION_MODE,
    is_workflow_complete,
    resolve_container_workdir,
    resolve_execution_mode,
    resolve_poll_timeout_sec,
    sanitize_docker_compose_project_name,
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


def test_sanitize_docker_compose_project_name() -> None:
    assert (
        sanitize_docker_compose_project_name("hello-world__bZZeEkw__env")
        == "hello-world__bzzeekw__env"
    )
    assert sanitize_docker_compose_project_name("  My Task ") == "mytask"
    assert sanitize_docker_compose_project_name("9bad") == "p9bad"
    assert sanitize_docker_compose_project_name("---") == "p---"
    assert sanitize_docker_compose_project_name("@@@") == "harbor"
    assert len(sanitize_docker_compose_project_name("a" * 100)) == 63


def test_resolve_container_workdir_prefers_task_config(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class _EnvConfig:
        workdir = "/custom/task/root"

    class _Env:
        task_env_config = _EnvConfig()

    monkeypatch.setattr(
        "benchmarks.harbor.libragent_agent.docker_inspect_workdir",
        lambda _cid: "/should-not-use",
    )
    assert resolve_container_workdir(_Env(), container_id="cid") == "/custom/task/root"


def test_resolve_container_workdir_uses_image_workdir_when_task_omits(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class _EnvConfig:
        workdir = None

    class _Env:
        task_env_config = _EnvConfig()

    monkeypatch.setattr(
        "benchmarks.harbor.libragent_agent.docker_inspect_workdir",
        lambda _cid: "/workspace",
    )
    monkeypatch.setattr(
        "benchmarks.harbor.libragent_agent.docker_exec_pwd",
        lambda _cid: "/should-not-use",
    )
    assert resolve_container_workdir(_Env(), container_id="cid") == "/workspace"


def test_resolve_container_workdir_uses_live_pwd_when_inspect_empty(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class _EnvConfig:
        workdir = None

    class _Env:
        task_env_config = _EnvConfig()

    monkeypatch.setattr(
        "benchmarks.harbor.libragent_agent.docker_inspect_workdir",
        lambda _cid: None,
    )
    monkeypatch.setattr(
        "benchmarks.harbor.libragent_agent.docker_exec_pwd",
        lambda _cid: "/home/agent",
    )
    assert resolve_container_workdir(_Env(), container_id="cid") == "/home/agent"


def test_resolve_container_workdir_falls_back_to_app_with_warning(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    class _EnvConfig:
        workdir = None

    class _Env:
        task_env_config = _EnvConfig()

    monkeypatch.setattr(
        "benchmarks.harbor.libragent_agent.docker_inspect_workdir",
        lambda _cid: None,
    )
    monkeypatch.setattr(
        "benchmarks.harbor.libragent_agent.docker_exec_pwd",
        lambda _cid: None,
    )
    assert resolve_container_workdir(_Env(), container_id="cid") == "/app"


def test_attach_session_payload_shape() -> None:
    """Document expected dockerConfig when Harbor main container is resolved."""
    payload = {
        "workspaceIsolation": "docker",
        "dockerConfig": {
            "attachContainer": "cid123",
            "workdir": "/workspace",
            "manageLifecycle": False,
        },
    }
    assert payload["dockerConfig"]["manageLifecycle"] is False
    assert payload["dockerConfig"]["workdir"] == "/workspace"
    assert "image" not in payload["dockerConfig"]
