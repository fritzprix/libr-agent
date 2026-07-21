"""Harbor Framework adapter for LibrAgent.

Bridges Harbor's terminal-benchmark loop to LibrAgent's headless Session API.

When the Harbor environment is a local Docker Compose trial, the adapter attaches
LibrAgent's Docker session to Harbor's existing main container (workdir usually
`/app`) so absolute task paths work without host↔container sync.

Non-Docker Harbor backends fall back to the legacy host-workspace sync path.
"""

from __future__ import annotations

import asyncio
import os
import re
import subprocess
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


def resolve_container_workdir(environment: BaseEnvironment) -> str:
    workdir = "/app"
    if hasattr(environment, "task_env_config") and environment.task_env_config:
        workdir = getattr(environment.task_env_config, "workdir", None) or "/app"
    return workdir


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
        """Validates connection with the running LibrAgent daemon."""
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

        container_workdir = resolve_container_workdir(environment)
        attach_container_id = resolve_harbor_main_container_id(environment)
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
                    current_status = str(session_info.get("status", "idle")).lower()
                    last_status = current_status
                    if current_status not in TERMINAL_WORKFLOW_STATUSES:
                        seen_non_idle = True

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
                    f"while status={last_status}. Refusing to harvest incomplete results."
                )
                raise

            if not completed:
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

            context.metadata = {
                "output": final_answer,
                "trajectory": messages,
                "sessionId": session_id,
                "finalStatus": last_status,
                "completed": True,
                "attachContainer": attach_container_id,
                "workspaceMode": "attach" if use_attach else "host-sync",
            }
            print(
                f"[{self.name()}] Task complete. Response harvested successfully "
                f"({len(messages)} messages, status={last_status})."
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
