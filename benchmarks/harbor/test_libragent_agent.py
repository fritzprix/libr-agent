from __future__ import annotations

import pytest

from benchmarks.harbor.libragent_agent import (
    DEFAULT_EXECUTION_MODE,
    LibrAgentHarborAdapter,
    extract_model_name_from_assistant_payload,
    extract_model_name_from_session_payload,
    extract_trajectory_error,
    format_harbor_model_name,
    is_workflow_complete,
    resolve_container_workdir,
    resolve_execution_mode,
    resolve_poll_timeout_sec,
    sanitize_docker_compose_project_name,
    split_harbor_model_name,
    summarize_trajectory,
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


def test_format_harbor_model_name() -> None:
    assert format_harbor_model_name(None) is None
    assert format_harbor_model_name("  ") is None
    assert format_harbor_model_name("gpt-5.4") == "gpt-5.4"
    assert format_harbor_model_name("gpt-5.4", "openai") == "openai/gpt-5.4"
    assert (
        format_harbor_model_name("openrouter/foo", "openrouter") == "openrouter/foo"
    )


def test_extract_model_name_from_assistant_payload() -> None:
    assert (
        extract_model_name_from_assistant_payload(
            {"config": {"model": "gpt-5.4", "provider": "openai"}}
        )
        == "openai/gpt-5.4"
    )
    assert extract_model_name_from_assistant_payload({"config": {}}) is None
    assert extract_model_name_from_assistant_payload(None) is None


def test_extract_model_name_from_assistant_payload_parses_json_string_config() -> None:
    """The assistants API serializes ``config`` as JSON text, not an object."""
    payload = {
        "id": "coder",
        "config": '{"model": "gpt-5.4", "provider": "openai", "mcpServerIds": []}',
    }
    assert extract_model_name_from_assistant_payload(payload) == "openai/gpt-5.4"


def test_extract_model_name_from_assistant_payload_tolerates_broken_config() -> None:
    assert extract_model_name_from_assistant_payload({"config": "not json"}) is None
    assert (
        extract_model_name_from_assistant_payload({"config": '{"mcpServerIds": []}'})
        is None
    )


def test_extract_model_name_from_session_payload() -> None:
    assert (
        extract_model_name_from_session_payload(
            {"model": "claude-sonnet-4", "provider": "anthropic"}
        )
        == "anthropic/claude-sonnet-4"
    )


def test_split_harbor_model_name() -> None:
    assert split_harbor_model_name("openai/gpt-5.4") == ("openai", "gpt-5.4")
    assert split_harbor_model_name("gpt-5.4") == (None, "gpt-5.4")
    assert split_harbor_model_name(None) == (None, None)
    assert split_harbor_model_name("  ") == (None, None)


def _make_adapter(tmp_path, model_name: str | None = None) -> LibrAgentHarborAdapter:
    return LibrAgentHarborAdapter(logs_dir=tmp_path, model_name=model_name)


def test_to_agent_info_returns_stable_instance(tmp_path) -> None:
    adapter = _make_adapter(tmp_path)
    assert adapter.to_agent_info() is adapter.to_agent_info()


def test_apply_model_name_backfills_captured_agent_info(tmp_path) -> None:
    """Harbor snapshots agent_info before run but serializes it after."""
    adapter = _make_adapter(tmp_path)
    captured = adapter.to_agent_info()
    assert captured.model_info is None

    assert adapter._apply_model_name("openai/Qwen3.6-35B") is True

    assert captured.model_info is not None
    assert captured.model_info.name == "Qwen3.6-35B"
    assert captured.model_info.provider == "openai"


def test_apply_model_name_ignores_empty_and_unchanged(tmp_path) -> None:
    adapter = _make_adapter(tmp_path, model_name="openai/gpt-5.4")
    assert adapter._apply_model_name(None) is False
    assert adapter._apply_model_name("openai/gpt-5.4") is False
    assert adapter.to_agent_info().model_info.name == "gpt-5.4"


def test_summarize_trajectory_sums_usage_and_tool_calls() -> None:
    messages = [
        {"role": "user", "content": [{"type": "text", "text": "hi"}], "usage": None},
        {
            "role": "assistant",
            "content": [{"type": "text", "text": "working"}],
            "toolCalls": [{"id": "1", "name": "shell"}],
            "usage": {
                "promptTokens": 100,
                "completionTokens": 10,
                "cachedPromptTokens": 40,
            },
        },
        {
            "role": "assistant",
            "content": [{"type": "text", "text": "done"}],
            "toolCalls": [{"id": "2", "name": "shell"}, {"id": "3", "name": "read"}],
            "usage": {
                "promptTokens": 200,
                "completionTokens": 20,
                "cachedPromptTokens": 150,
            },
        },
    ]
    telemetry = summarize_trajectory(messages)
    assert telemetry.has_usage is True
    assert telemetry.n_input_tokens == 300
    assert telemetry.n_output_tokens == 30
    assert telemetry.n_cache_tokens == 190
    assert telemetry.n_turns == 2
    assert telemetry.tool_calls_count == 3
    assert telemetry.error is None


def test_summarize_trajectory_without_usage_leaves_tokens_none() -> None:
    messages = [
        {"role": "user", "content": [{"type": "text", "text": "hi"}]},
        {"role": "assistant", "content": [{"type": "text", "text": "bye"}], "toolCalls": []},
    ]
    telemetry = summarize_trajectory(messages)
    assert telemetry.has_usage is False
    assert telemetry.n_input_tokens is None
    assert telemetry.n_output_tokens is None
    assert telemetry.n_cache_tokens is None
    assert telemetry.n_turns == 1
    assert telemetry.tool_calls_count == 0


def test_extract_trajectory_error_prefers_latest_assistant_error() -> None:
    messages = [
        {"role": "user", "error": "ignored user error"},
        {"role": "assistant", "error": {"message": "429 quota exceeded"}},
        {"role": "assistant", "error": None},
        {"role": "tool", "error": "tool timeout"},
    ]
    assert extract_trajectory_error(messages) == "tool timeout"
    assert (
        extract_trajectory_error(
            [
                {"role": "user", "error": "user side"},
                {"role": "assistant", "error": {"message": "model overloaded"}},
            ]
        )
        == "model overloaded"
    )
