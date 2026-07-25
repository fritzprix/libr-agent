"""Harbor Framework adapter for LibrAgent.

Bridges Harbor's terminal-benchmark loop to LibrAgent's headless Session API.

When the Harbor environment is a local Docker Compose trial, the adapter attaches
LibrAgent's Docker session to Harbor's existing main container. Workdir is taken
from task config when set; otherwise from the container image WORKDIR (often
`/workspace` for Harbor Index / BixBench, `/app` for classic Terminal-Bench).

Non-Docker Harbor backends fall back to the legacy host-workspace sync path.
"""

from __future__ import annotations

import asyncio
import contextlib
import json
import os
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any

try:
    from harbor.agents.base import BaseAgent
    from harbor.environments.base import BaseEnvironment
    from harbor.models.agent.context import AgentContext
except ImportError as exc:  # pragma: no cover - import-time guard for editors
    raise ImportError(
        "The 'harbor' package is required for LibrAgentHarborAdapter. "
        "Install it via 'pip install harbor'."
    ) from exc

try:
    import httpx
except ImportError:  # pragma: no cover
    httpx = None  # type: ignore[assignment]

EXECUTION_MODE_VALUES = ("normal", "yolo", "unsafe")
DEFAULT_EXECUTION_MODE = "unsafe"
# Idle/error end the workflow. Paused means approval wait — not done for benchmarks.
TERMINAL_WORKFLOW_STATUSES = frozenset({"idle", "error"})
DEFAULT_POLL_INTERVAL_SEC = 3.0
MAIN_COMPOSE_SERVICE = "main"


@dataclass(frozen=True)
class TrajectoryTelemetry:
    """Aggregated Harbor-facing metrics harvested from LibrAgent messages."""

    n_input_tokens: int | None
    n_output_tokens: int | None
    n_cache_tokens: int | None
    n_turns: int
    tool_calls_count: int
    has_usage: bool
    error: str | None


def _as_non_negative_int(value: Any) -> int | None:
    """Coerce JSON number-like values to a non-negative int, else None."""
    if value is None or isinstance(value, bool):
        return None
    if isinstance(value, int):
        return value if value >= 0 else None
    if isinstance(value, float):
        if value < 0 or not value.is_integer():
            return None
        return int(value)
    if isinstance(value, str):
        text = value.strip()
        if not text:
            return None
        try:
            parsed = int(text)
        except ValueError:
            return None
        return parsed if parsed >= 0 else None
    return None


def _usage_token(usage: Any, *keys: str) -> int | None:
    if not isinstance(usage, dict):
        return None
    for key in keys:
        parsed = _as_non_negative_int(usage.get(key))
        if parsed is not None:
            return parsed
    return None


def format_harbor_model_name(
    model: str | None,
    provider: str | None = None,
) -> str | None:
    """Build Harbor ``provider/model`` form when both sides are known."""
    model_name = (model or "").strip()
    if not model_name:
        return None
    provider_name = (provider or "").strip()
    if not provider_name:
        return model_name
    if "/" in model_name:
        # Already provider-qualified (e.g. openrouter/...).
        return model_name
    return f"{provider_name}/{model_name}"


def extract_model_name_from_assistant_payload(payload: Any) -> str | None:
    """Read model/provider from ``GET /assistants/:id`` JSON."""
    if not isinstance(payload, dict):
        return None
    config = payload.get("config")
    if not isinstance(config, dict):
        config = {}
    model = payload.get("model") or config.get("model")
    provider = payload.get("provider") or config.get("provider")
    if not isinstance(model, str):
        model = None
    if not isinstance(provider, str):
        provider = None
    return format_harbor_model_name(model, provider)


def extract_model_name_from_session_payload(payload: Any) -> str | None:
    """Read model/provider from ``GET /sessions/:id`` JSON."""
    if not isinstance(payload, dict):
        return None
    model = payload.get("model")
    provider = payload.get("provider")
    if not isinstance(model, str):
        model = None
    if not isinstance(provider, str):
        provider = None
    return format_harbor_model_name(model, provider)


def _format_message_error(error: Any) -> str | None:
    if error is None:
        return None
    if isinstance(error, str):
        text = error.strip()
        return text or None
    if isinstance(error, dict):
        for key in ("message", "error", "reason", "detail", "code"):
            value = error.get(key)
            if isinstance(value, str) and value.strip():
                return value.strip()
        try:
            return json.dumps(error, ensure_ascii=False, sort_keys=True)
        except (TypeError, ValueError):
            return str(error)
    return str(error)


def extract_trajectory_error(messages: list[Any]) -> str | None:
    """Prefer the latest assistant/tool message error, else any message error."""
    last_any: str | None = None
    last_preferred: str | None = None
    for message in messages:
        if not isinstance(message, dict):
            continue
        formatted = _format_message_error(message.get("error"))
        if not formatted:
            continue
        last_any = formatted
        role = str(message.get("role") or "").lower()
        if role in {"assistant", "tool", "system"}:
            last_preferred = formatted
    return last_preferred or last_any


def summarize_trajectory(messages: list[Any]) -> TrajectoryTelemetry:
    """Aggregate token/turn/tool metrics from harvested LibrAgent messages.

    Harbor ``n_input_tokens`` is total input *including* cache. LibrAgent's
    ``usage.promptTokens`` already includes cached prompt tokens;
    ``usage.cachedPromptTokens`` is recorded separately as ``n_cache_tokens``.
    """
    n_input = 0
    n_output = 0
    n_cache = 0
    has_usage = False
    n_turns = 0
    tool_calls_count = 0

    for message in messages:
        if not isinstance(message, dict):
            continue
        role = str(message.get("role") or "").lower()
        if role == "assistant":
            n_turns += 1
            tool_calls = message.get("toolCalls")
            if isinstance(tool_calls, list):
                tool_calls_count += len(tool_calls)

        usage = message.get("usage")
        prompt = _usage_token(usage, "promptTokens", "prompt_tokens", "input_tokens")
        completion = _usage_token(
            usage, "completionTokens", "completion_tokens", "output_tokens"
        )
        cached = _usage_token(
            usage,
            "cachedPromptTokens",
            "cached_prompt_tokens",
            "cache_read_input_tokens",
            "cached_tokens",
        )
        if prompt is None and completion is None and cached is None:
            continue
        has_usage = True
        n_input += prompt or 0
        n_output += completion or 0
        n_cache += cached or 0

    return TrajectoryTelemetry(
        n_input_tokens=n_input if has_usage else None,
        n_output_tokens=n_output if has_usage else None,
        n_cache_tokens=n_cache if has_usage else None,
        n_turns=n_turns,
        tool_calls_count=tool_calls_count,
        has_usage=has_usage,
        error=extract_trajectory_error(messages),
    )


def resolve_execution_mode(
    explicit_mode: str | None = None,
    *,
    env: dict[str, str] | None = None,
) -> str:
    """Resolve benchmark execution mode from explicit arg or LIBRAGENT_EXECUTION_MODE."""
    env_map = env if env is not None else os.environ
    candidate = (explicit_mode or env_map.get("LIBRAGENT_EXECUTION_MODE") or DEFAULT_EXECUTION_MODE)
    mode = candidate.strip().lower()
    if mode not in EXECUTION_MODE_VALUES:
        allowed = ", ".join(EXECUTION_MODE_VALUES)
        raise ValueError(
            f"Invalid execution mode '{candidate}'. Expected one of: {allowed}."
        )
    return mode


def is_workflow_complete(status: str, *, seen_non_idle: bool) -> bool:
    """Return True only when session status is a real workflow completion.

    Harbor must not harvest while Busy/Queued/Provisioning/Paused. Brief Idle
    before Busy is ignored until a non-idle status has been observed.
    """
    normalized = status.strip().lower()
    if normalized not in TERMINAL_WORKFLOW_STATUSES:
        return False
    if normalized == "idle" and not seen_non_idle:
        return False
    return True


def resolve_poll_timeout_sec(
    explicit_timeout: float | None = None,
    *,
    env: dict[str, str] | None = None,
) -> float | None:
    """Optional wall-clock poll budget (seconds). None = wait until Harbor cancels."""
    env_map = env if env is not None else os.environ
    if explicit_timeout is not None:
        return float(explicit_timeout)
    raw = env_map.get("LIBRAGENT_POLL_TIMEOUT_SEC")
    if raw is None or raw.strip() == "":
        return None
    return float(raw)


def sanitize_docker_compose_project_name(name: str) -> str:
    """Mirror Harbor's compose project-name sanitizer."""
    sanitized = re.sub(r"[^a-zA-Z0-9_-]", "", name.lower().replace(" ", ""))
    if not sanitized:
        return "harbor"
    if sanitized[0].isdigit() or sanitized[0] == "-":
        sanitized = f"p{sanitized}"
    return sanitized[:63]


def docker_inspect_workdir(container_id: str) -> str | None:
    """Read the container's configured WorkingDir (image WORKDIR), if any."""
    try:
        result = subprocess.run(
            [
                "docker",
                "inspect",
                "-f",
                "{{.Config.WorkingDir}}",
                container_id,
            ],
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        print(
            f"[LibrAgent] Warning: docker inspect workdir failed for "
            f"{container_id!r}: {exc}"
        )
        return None
    if result.returncode != 0:
        stderr = (result.stderr or "").strip()
        print(
            f"[LibrAgent] Warning: docker inspect workdir failed for "
            f"{container_id!r}: {stderr or result.returncode}"
        )
        return None
    workdir = (result.stdout or "").strip()
    if not workdir or workdir == "/":
        return None
    return workdir


def docker_exec_pwd(container_id: str) -> str | None:
    """Ask the live container for its current working directory."""
    try:
        result = subprocess.run(
            ["docker", "exec", container_id, "pwd"],
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        print(
            f"[LibrAgent] Warning: docker exec pwd failed for "
            f"{container_id!r}: {exc}"
        )
        return None
    if result.returncode != 0:
        return None
    workdir = (result.stdout or "").strip()
    if not workdir or workdir == "/":
        return None
    return workdir


def resolve_container_workdir(
    environment: BaseEnvironment,
    container_id: str | None = None,
) -> str:
    """Resolve the container path LibrAgent should treat as workspace root.

    Workdir is **per-task / per-image**, never a single hardcoded benchmark path.

    Priority:
    1. Harbor task ``[environment].workdir`` when the task sets it explicitly
    2. Docker image WORKDIR of the attached Harbor main container
    3. Live ``docker exec … pwd`` (container process cwd)
    4. Last-resort ``/app`` with a warning (legacy Terminal-Bench convention only)
    """
    if hasattr(environment, "task_env_config") and environment.task_env_config:
        configured = getattr(environment.task_env_config, "workdir", None)
        if configured:
            workdir = str(configured)
            print(f"[LibrAgent] Using task-configured container workdir: {workdir}")
            return workdir

    if container_id:
        inspected = docker_inspect_workdir(container_id)
        if inspected:
            print(
                f"[LibrAgent] Using container image WORKDIR for attach: {inspected}"
            )
            return inspected

        live_pwd = docker_exec_pwd(container_id)
        if live_pwd:
            print(f"[LibrAgent] Using live container pwd for attach: {live_pwd}")
            return live_pwd

    print(
        "[LibrAgent] Warning: no task workdir / image WORKDIR / live pwd found; "
        "falling back to /app (may be wrong for this task)"
    )
    return "/app"


def resolve_harbor_main_container_id(environment: BaseEnvironment) -> str | None:
    """Resolve Harbor's Docker Compose `main` container id, if available."""
    session_id = getattr(environment, "session_id", None)
    if not session_id:
        print(
            "[LibrAgent] Warning: Harbor environment has no session_id; "
            "cannot resolve Compose main container for attach."
        )
        return None

    project = sanitize_docker_compose_project_name(str(session_id))
    try:
        result = subprocess.run(
            [
                "docker",
                "ps",
                "-q",
                "--filter",
                f"label=com.docker.compose.project={project}",
                "--filter",
                f"label=com.docker.compose.service={MAIN_COMPOSE_SERVICE}",
            ],
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        print(
            f"[LibrAgent] Warning: docker ps failed while resolving Harbor main "
            f"container (project={project!r}): {exc}"
        )
        return None

    if result.returncode != 0:
        stderr = (result.stderr or "").strip()
        print(
            f"[LibrAgent] Warning: docker ps returned {result.returncode} while "
            f"resolving Harbor main container (project={project!r})"
            + (f": {stderr}" if stderr else "")
        )
        return None

    for line in (result.stdout or "").splitlines():
        cid = line.strip()
        if cid:
            return cid

    print(
        f"[LibrAgent] Warning: no running Compose service '{MAIN_COMPOSE_SERVICE}' "
        f"for project={project!r} (session_id={session_id!r}). "
        "If Harbor's sanitizer differs, attach will be skipped and host-sync fallback used."
    )
    return None


class LibrAgentHarborAdapter(BaseAgent):
    """
    Harbor Framework Adapter for LibrAgent.
    Integrates LibrAgent's headless REST API with Harbor's terminal benchmark loop.
    """

    def __init__(
        self,
        logs_dir: Path,
        model_name: str | None = None,
        logger: Any | None = None,
        mcp_servers: list[Any] | None = None,
        skills_dir: str | None = None,
        *args: Any,
        api_url: str = "http://localhost:3030/api",
        assistant_id: str = "coder-assistant",
        execution_mode: str | None = None,
        poll_timeout_sec: float | None = None,
        poll_interval_sec: float = DEFAULT_POLL_INTERVAL_SEC,
        extra_env: dict[str, str] | None = None,
        **kwargs: Any,
    ) -> None:
        self.api_url = api_url.rstrip("/")
        self.assistant_id = assistant_id
        self.execution_mode = resolve_execution_mode(execution_mode)
        self.poll_timeout_sec = resolve_poll_timeout_sec(poll_timeout_sec)
        self.poll_interval_sec = float(poll_interval_sec)
        super().__init__(
            logs_dir,
            model_name,
            logger,
            mcp_servers,
            skills_dir,
            *args,
            extra_env=extra_env,
            **kwargs,
        )

    @staticmethod
    def name() -> str:
        return "LibrAgent"

    def version(self) -> str | None:
        return "0.8.33"

    async def setup(self, environment: BaseEnvironment) -> None:
        """Validates connection and resolves Harbor model_info from the assistant."""
        _ = environment  # Harbor requires the arg; health/model lookup is API-only.
        if httpx is None:
            raise RuntimeError(
                "Python library 'httpx' is missing. Please run: pip install httpx"
            )

        async with httpx.AsyncClient() as client:
            try:
                res = await client.get(f"{self.api_url}/health", timeout=5.0)
                if res.status_code != 200:
                    raise RuntimeError(
                        f"LibrAgent health check returned invalid status code: {res.status_code}"
                    )
            except Exception as e:
                if isinstance(e, RuntimeError):
                    raise
                raise RuntimeError(
                    f"Unable to connect to LibrAgent daemon at {self.api_url}. "
                    f"Please make sure LibrAgent is running and API is active. Error: {e}"
                ) from e

            # Harbor records agent_info.model_info from self.model_name during
            # prepare (after setup, before run). Prefer an explicit Harbor -m /
            # constructor model_name; otherwise resolve from the assistant config.
            if not self.model_name:
                await self._resolve_model_name_from_assistant(client)

    async def _resolve_model_name_from_assistant(self, client: Any) -> None:
        """Populate ``self.model_name`` from ``GET /assistants/{id}`` when unset."""
        try:
            res = await client.get(
                f"{self.api_url}/assistants/{self.assistant_id}",
                timeout=10.0,
            )
        except Exception as e:
            print(
                f"[{self.name()}] Warning: failed to fetch assistant "
                f"{self.assistant_id} for model_info: {e}"
            )
            return

        if res.status_code != 200:
            print(
                f"[{self.name()}] Warning: assistant lookup for model_info returned "
                f"{res.status_code}: {res.text}"
            )
            return

        try:
            payload = res.json()
        except Exception as e:
            print(
                f"[{self.name()}] Warning: assistant payload was not JSON: {e}"
            )
            return

        model_name = extract_model_name_from_assistant_payload(payload)
        if not model_name:
            print(
                f"[{self.name()}] Warning: assistant {self.assistant_id} has no "
                f"model/provider in config; agent_info.model_info will stay null."
            )
            return

        self.model_name = model_name
        self._init_model_info()
        print(f"[{self.name()}] Resolved Harbor model_info from assistant: {model_name}")

    async def run(
        self,
        instruction: str,
        environment: BaseEnvironment,
        context: AgentContext,
    ) -> None:
        """
        Spawns a session in LibrAgent, injects the task instruction, polls for completion,
        and harvests the final trajectory and output.
        """
        if httpx is None:
            raise RuntimeError("Python library 'httpx' is missing.")

        container_id = resolve_harbor_main_container_id(environment)
        container_workdir = resolve_container_workdir(environment, container_id)
        attach_container_id = container_id
        use_attach = attach_container_id is not None

        local_workspace: Path | None = None
        local_workspace_str: str | None = None

        if use_attach:
            print(
                f"[{self.name()}] Attaching LibrAgent Docker session to Harbor "
                f"container {attach_container_id} (workdir={container_workdir})..."
            )
        else:
            local_workspace = self._resolve_local_workspace(environment, context)
            local_workspace_str = str(local_workspace.resolve().absolute())
            os.makedirs(local_workspace_str, exist_ok=True)
            print(
                f"[{self.name()}] Harbor container id not resolved; falling back to "
                f"host workspace sync ({local_workspace_str})."
            )
            print(
                f"[{self.name()}] Pulling initial container files from "
                f"{container_workdir} to {local_workspace_str}..."
            )
            try:
                await environment.download_dir(
                    source_dir=container_workdir,
                    target_dir=local_workspace_str,
                )
                print(f"[{self.name()}] Successfully pulled initial files.")
            except Exception as e:
                print(
                    f"[{self.name()}] Warning: failed to pull initial container files: {e}"
                )

        task_id = getattr(context, "task_id", None) or "bench"
        payload: dict[str, Any] = {
            "assistantId": self.assistant_id,
            "name": f"Harbor Benchmark Task: {task_id}",
            "request": instruction,
            "executionMode": self.execution_mode,
        }

        if use_attach and attach_container_id is not None:
            payload["workspaceIsolation"] = "docker"
            payload["dockerConfig"] = {
                "attachContainer": attach_container_id,
                "workdir": container_workdir,
                "manageLifecycle": False,
            }
        else:
            payload["workspacePath"] = local_workspace_str
            payload["workspaceIsolation"] = "host"

        async with httpx.AsyncClient(timeout=600.0) as client:
            mode_desc = (
                f"attach:{attach_container_id}"
                if use_attach
                else f"host:{local_workspace_str}"
            )
            print(f"[{self.name()}] Creating session ({mode_desc})...")
            res = await client.post(f"{self.api_url}/sessions", json=payload)
            if res.status_code != 201:
                raise RuntimeError(
                    f"LibrAgent session creation failed ({res.status_code}): {res.text}"
                )

            response_json = res.json()
            session_id = response_json.get("id")
            if not session_id:
                raise RuntimeError(
                    "LibrAgent session creation succeeded but response did not contain "
                    f"'id': {response_json}"
                )

            print(
                f"[{self.name()}] Session {session_id} spawned successfully "
                f"(executionMode={self.execution_mode}). Awaiting execution completion..."
            )

            # In attach/docker mode, download initial container files to host staging workspace
            # so that host-based file search tools (like globFiles, grepFiles) can find them.
            if use_attach:
                try:
                    session_res = await client.get(f"{self.api_url}/sessions/{session_id}")
                    if session_res.status_code == 200:
                        session_info = session_res.json()
                        host_workspace = session_info.get("dockerHostWorkspacePath")
                        if host_workspace:
                            host_workspace_path = Path(host_workspace)
                            os.makedirs(host_workspace_path, exist_ok=True)
                            print(
                                f"[{self.name()}] Pulling initial container files from "
                                f"{container_workdir} to host staging workspace ({host_workspace})..."
                            )
                            await environment.download_dir(
                                source_dir=container_workdir,
                                target_dir=str(host_workspace_path.resolve().absolute()),
                            )
                            print(f"[{self.name()}] Successfully pulled initial files to host staging workspace.")
                except Exception as e:
                    print(
                        f"[{self.name()}] Warning: failed to pull initial container files to host staging: {e}"
                    )
            if self.poll_timeout_sec is not None:
                print(
                    f"[{self.name()}] Poll wall-clock budget: "
                    f"{self.poll_timeout_sec:.0f}s"
                )

            poll_interval = self.poll_interval_sec
            seen_non_idle = False
            completed = False
            last_status = "unknown"
            last_session_info: dict[str, Any] | None = None
            poll_deadline = (
                asyncio.get_running_loop().time() + self.poll_timeout_sec
                if self.poll_timeout_sec is not None
                else None
            )

            try:
                while True:
                    if (
                        poll_deadline is not None
                        and asyncio.get_running_loop().time() >= poll_deadline
                    ):
                        await self._terminate_session(session_id)
                        raise TimeoutError(
                            f"LibrAgent session {session_id} did not reach a terminal "
                            f"workflow status within {self.poll_timeout_sec:.0f}s "
                            f"(last status={last_status}). Increase Harbor "
                            f"--agent-timeout-multiplier / LIBRAGENT_POLL_TIMEOUT_SEC "
                            f"or wait for the agent to finish before harvesting."
                        )

                    await asyncio.sleep(poll_interval)
                    try:
                        status_res = await client.get(
                            f"{self.api_url}/sessions/{session_id}"
                        )
                    except Exception as e:
                        print(
                            f"[{self.name()}] Connection error during polling, retrying: {e}"
                        )
                        continue

                    if status_res.status_code != 200:
                        print(
                            f"[{self.name()}] Warning: Failed to fetch session status: "
                            f"{status_res.text}"
                        )
                        continue

                    session_info = status_res.json()
                    if isinstance(session_info, dict):
                        last_session_info = session_info
                    current_status = str(session_info.get("status", "idle")).lower()
                    last_status = current_status
                    if current_status not in TERMINAL_WORKFLOW_STATUSES:
                        seen_non_idle = True

                    if current_status == "paused":
                        print(
                            f"[{self.name()}] Session workflow was paused (or cancelled). "
                            f"Terminating session and failing fast."
                        )
                        await self._terminate_session(session_id)
                        raise RuntimeError(
                            f"LibrAgent session {session_id} was paused/cancelled."
                        )

                    if is_workflow_complete(
                        current_status, seen_non_idle=seen_non_idle
                    ):
                        print(
                            f"[{self.name()}] Session workflow reached terminal state: "
                            f"{current_status}"
                        )
                        completed = True
                        break
            except asyncio.CancelledError:
                # Harbor agent timeout cancels this coroutine. Do NOT harvest as
                # success — that caused verifiers to score incomplete workspaces.
                print(
                    f"[{self.name()}] Session polling cancelled (Harbor timeout) "
                    f"while status={last_status}. Terminating session and refusing "
                    f"to harvest incomplete results."
                )
                # Shielded terminate re-raises CancelledError after teardown.
                await self._terminate_session(session_id)
                raise

            if not completed:
                await self._terminate_session(session_id)
                raise RuntimeError(
                    f"LibrAgent session {session_id} ended polling without a completed "
                    f"workflow (last status={last_status})."
                )

            if not use_attach and local_workspace is not None:
                print(
                    f"[{self.name()}] Pushing updated workspace files from "
                    f"{local_workspace} to container {container_workdir}..."
                )
                try:
                    await self._upload_workspace(
                        environment,
                        local_workspace,
                        container_workdir,
                    )
                    print(f"[{self.name()}] Successfully pushed workspace files.")
                except Exception as e:
                    await self._terminate_session(session_id)
                    raise RuntimeError(
                        f"Error pushing workspace files back to container: {e}"
                    ) from e
            elif use_attach:
                print(
                    f"[{self.name()}] Attach mode: skipping host→container upload "
                    f"(agent wrote in-place under {container_workdir})."
                )
                local_workspace = self._resolve_local_workspace(environment, context)
                local_workspace_str = str(local_workspace.resolve().absolute())
                os.makedirs(local_workspace_str, exist_ok=True)
                print(
                    f"[{self.name()}] Pulling final container files from "
                    f"{container_workdir} to local workspace ({local_workspace_str})..."
                )
                try:
                    await environment.download_dir(
                        source_dir=container_workdir,
                        target_dir=local_workspace_str,
                    )
                    print(f"[{self.name()}] Successfully pulled final files to local workspace.")
                except Exception as e:
                    print(
                        f"[{self.name()}] Warning: failed to pull final container files: {e}"
                    )

            messages_res = await client.get(
                f"{self.api_url}/sessions/{session_id}/messages"
            )
            if messages_res.status_code != 200:
                print(
                    f"[{self.name()}] Warning: Failed to harvest session messages "
                    f"({messages_res.status_code})"
                )
                messages: list[Any] = []
            else:
                messages_data = messages_res.json()
                messages = messages_data.get("messages", [])

            final_answer = ""
            for msg in reversed(messages):
                if msg.get("role") != "assistant":
                    continue
                contents = msg.get("content", [])
                final_answer = "\n".join(
                    item.get("text", "")
                    for item in contents
                    if item.get("type") == "text"
                )
                break

            telemetry = summarize_trajectory(messages)
            # Harbor result.json reads these top-level AgentContext fields — not metadata.
            context.n_input_tokens = telemetry.n_input_tokens
            context.n_output_tokens = telemetry.n_output_tokens
            context.n_cache_tokens = telemetry.n_cache_tokens
            # LibrAgent does not currently expose USD cost in message usage.
            context.cost_usd = None

            session_model_name = extract_model_name_from_session_payload(
                last_session_info
            )
            metadata: dict[str, Any] = {
                "output": final_answer,
                "trajectory": messages,
                "sessionId": session_id,
                "finalStatus": last_status,
                "completed": True,
                "attachContainer": attach_container_id,
                "workspaceMode": "attach" if use_attach else "host-sync",
                "assistant_id": self.assistant_id,
                "n_turns": telemetry.n_turns,
                "tool_calls_count": telemetry.tool_calls_count,
            }
            resolved_model = session_model_name or self.model_name
            if resolved_model:
                metadata["model_name"] = resolved_model
            if last_status in {"error", "cancel", "cancelled"} and telemetry.error:
                metadata["error"] = telemetry.error
            context.metadata = metadata
            print(
                f"[{self.name()}] Task complete. Response harvested successfully "
                f"({len(messages)} messages, status={last_status}, "
                f"turns={telemetry.n_turns}, tools={telemetry.tool_calls_count}, "
                f"tokens_in={telemetry.n_input_tokens}, "
                f"tokens_out={telemetry.n_output_tokens}, "
                f"tokens_cache={telemetry.n_cache_tokens})."
            )

            # Harvest finished; release the LibrAgent session so it does not keep
            # running (and, in attach mode, keep writing into Harbor's container)
            # after Harbor moves on to the next task.
            await self._terminate_session(session_id)

    async def _terminate_session(self, session_id: str) -> None:
        """Terminate a LibrAgent session, surviving Harbor's coroutine cancellation.

        The terminate request is shielded so a Harbor agent-timeout cancel still
        tears the session down instead of orphaning it (which would otherwise keep
        the workflow running and, in attach mode, keep mutating Harbor's container).
        When the caller is being cancelled, CancelledError is re-raised only after
        the terminate request completes, preserving abort semantics.
        """
        task = asyncio.ensure_future(self._terminate_session_best_effort(session_id))
        try:
            await asyncio.shield(task)
        except asyncio.CancelledError:
            with contextlib.suppress(BaseException):
                await task
            raise

    async def _terminate_session_best_effort(self, session_id: str) -> None:
        """POST /sessions/{id}/terminate, logging and swallowing transport errors.

        Uses a dedicated short-lived client because the primary request client may
        already be closing while the surrounding coroutine unwinds.
        """
        if httpx is None:
            return
        try:
            async with httpx.AsyncClient(timeout=30.0) as client:
                res = await client.post(
                    f"{self.api_url}/sessions/{session_id}/terminate"
                )
            if res.status_code == 200:
                print(f"[{self.name()}] Terminated LibrAgent session {session_id}.")
            else:
                print(
                    f"[{self.name()}] Warning: terminate for session {session_id} "
                    f"returned {res.status_code}: {res.text}"
                )
        except Exception as e:
            print(
                f"[{self.name()}] Warning: failed to terminate session "
                f"{session_id}: {e}"
            )

    def _resolve_local_workspace(
        self, environment: BaseEnvironment, context: AgentContext
    ) -> Path:
        trial_paths = getattr(environment, "trial_paths", None)
        if (
            trial_paths
            and hasattr(trial_paths, "trial_dir")
            and trial_paths.trial_dir
        ):
            return Path(trial_paths.trial_dir) / "workspace"

        task_id = getattr(context, "task_id", None) or "benchmark"
        return Path(os.getcwd()) / f"harbor-workspace-{task_id}"

    async def _upload_workspace(
        self,
        environment: BaseEnvironment,
        local_workspace: Path,
        container_workdir: str,
    ) -> None:
        """Upload host workspace files into the container workdir.

        Prefer per-file uploads so Windows host paths and LibrAgent metadata
        (``.libragent``) do not break ``docker compose cp``.
        """
        uploaded = 0
        for path in sorted(local_workspace.rglob("*")):
            if not path.is_file():
                continue
            rel = path.relative_to(local_workspace)
            if rel.parts and rel.parts[0] in {".libragent", ".git"}:
                continue
            target = f"{container_workdir.rstrip('/')}/{rel.as_posix()}"
            await environment.upload_file(str(path), target)
            uploaded += 1

        if uploaded == 0:
            # Fallback: whole-directory sync (may still help non-Windows hosts).
            await environment.upload_dir(
                source_dir=str(local_workspace),
                target_dir=container_workdir,
            )
