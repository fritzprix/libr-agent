"""Isolated LibrAgent instance for Harbor benchmark runs.

Benchmarks that install MCP servers or skills mutate the LibrAgent host. Running
them against the primary profile leaks state across runs (an "install X" task
passes or fails depending on what earlier runs or the user left behind) and can
touch the user's real configuration.

This module builds a pruned *seed* database from the primary profile once, then
launches the app binary against a fresh copy of that seed for every job:

    python -m benchmarks.harbor.isolated_instance seed \
        --assistant-id <uuid> --port 3031
    python -m benchmarks.harbor.isolated_instance launch --app-bin src-tauri/target/release/libragent
    pnpm bench:layman --api-url http://localhost:3031/api --assistant-id <uuid>
    python -m benchmarks.harbor.isolated_instance stop

The source database is only ever opened read-only.
"""

from __future__ import annotations

import argparse
import contextlib
import json
import os
import shutil
import signal
import socket
import sqlite3
import subprocess
import sys
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path

APP_IDENTIFIER = "com.fritzprix.libragent"
DB_FILE_NAME = "libragent_v2.db"
DEFAULT_PORT = 3031


def safe_print(*args: object, **kwargs: object) -> None:
    """``print`` that survives a closed or broken stdout/stderr.

    When the controlling terminal goes away every ``print`` raises
    ``OSError: [Errno 5]``; diagnostics must not kill a trial or a run.
    """
    try:
        print(*args, **kwargs)  # type: ignore[call-overload]
    except (OSError, ValueError):
        pass
DEFAULT_ROOT = Path(".libragent/work/bench-instance")
# Set by `run-harbor-bench.cjs --isolated`; the Harbor adapter relaunches the
# instance from the seed before every trial when the app binary is given.
INSTANCE_APP_BIN_ENV = "LIBRAGENT_BENCH_INSTANCE_APP_BIN"
INSTANCE_ROOT_ENV = "LIBRAGENT_BENCH_INSTANCE_ROOT"
# "1" runs the instance in a bubblewrap sandbox for host-workspace sessions: the
# filesystem is read-only except the run dir, HOME and /tmp are throwaway, and
# the run dir's app/ is mounted at /app (the workdir host-mode tasks refer to).
INSTANCE_SANDBOX_ENV = "LIBRAGENT_BENCH_INSTANCE_SANDBOX"
SANDBOX_WORKDIR = "/app"
DEFAULT_LAUNCH_TIMEOUT_SEC = 120.0


class InstanceError(RuntimeError):
    """Bench instance cannot be seeded, launched, or stopped."""

# Child tables before parents so deletes never depend on FK cascade settings.
RUNTIME_STATE_TABLES: tuple[str, ...] = (
    "pending_queue",
    "compact_contexts",
    "messages",
    "message_index_meta",
    "planning_todos",
    "planning_goals",
    "planning_scratchpad",
    "sessions",
    "chunks",
    "contents",
    "stores",
    "scheduled_tasks",
    "knowledge_chunk_entities",
    "knowledge_relationships",
    "knowledge_entities",
    "knowledge_chunks_v2",
    "knowledge",
)

# External-content FTS5 indexes rebuilt after their base tables are emptied.
# `knowledge_vectors` (vec0) cannot be edited without the sqlite-vec extension;
# orphaned vectors are harmless because search inner-joins chunks to vectors and
# chunk ids are AUTOINCREMENT (never reused).
FTS_INDEXES: tuple[str, ...] = ("knowledge_fts", "knowledge_chunks_fts")


@dataclass(frozen=True)
class InstanceLayout:
    root: Path

    @property
    def seed_db(self) -> Path:
        return self.root / "seed" / DB_FILE_NAME

    @property
    def seed_meta(self) -> Path:
        return self.root / "seed" / "seed.json"

    @property
    def run_dir(self) -> Path:
        return self.root / "run"

    @property
    def xdg_data(self) -> Path:
        return self.run_dir / "xdg-data"

    @property
    def home_dir(self) -> Path:
        return self.run_dir / "home"

    @property
    def tmp_dir(self) -> Path:
        return self.run_dir / "tmp"

    @property
    def app_dir(self) -> Path:
        """Host side of the sandbox's ``/app`` workspace."""
        return self.run_dir / "app"

    @property
    def data_dir(self) -> Path:
        return self.xdg_data / APP_IDENTIFIER

    @property
    def db_path(self) -> Path:
        return self.data_dir / DB_FILE_NAME

    @property
    def pid_file(self) -> Path:
        return self.root / "instance.pid"

    @property
    def log_file(self) -> Path:
        return self.root / "instance.log"

    @property
    def llm_settings_file(self) -> Path:
        """Secret-free record of the LLM settings the last launch ran with."""
        return self.root / "llm_settings.json"


def default_source_db() -> Path:
    data_home = os.environ.get("XDG_DATA_HOME") or str(Path.home() / ".local" / "share")
    return Path(data_home) / APP_IDENTIFIER / DB_FILE_NAME


def _existing_tables(conn: sqlite3.Connection) -> set[str]:
    rows = conn.execute("SELECT name FROM sqlite_master WHERE type = 'table'").fetchall()
    return {row[0] for row in rows}


def referenced_mcp_server_ids(conn: sqlite3.Connection, assistant_id: str) -> list[str]:
    row = conn.execute("SELECT config FROM assistants WHERE id = ?", (assistant_id,)).fetchone()
    if row is None:
        raise InstanceError(f"Assistant '{assistant_id}' not found in source database")
    config = json.loads(row[0] or "{}")
    ids = config.get("mcpServerIds") or []
    return [server_id for server_id in ids if isinstance(server_id, str)]


def prune_seed(conn: sqlite3.Connection, *, assistant_id: str, port: int) -> dict[str, object]:
    """Strip runtime/user state from a seed copy. Returns a summary for seed.json."""
    tables = _existing_tables(conn)
    kept_servers = referenced_mcp_server_ids(conn, assistant_id)

    cleared: dict[str, int] = {}
    for table in RUNTIME_STATE_TABLES:
        if table in tables:
            cleared[table] = conn.execute(f'DELETE FROM "{table}"').rowcount

    placeholders = ",".join("?" for _ in kept_servers)
    if kept_servers:
        removed = conn.execute(
            f"DELETE FROM mcp_servers WHERE id NOT IN ({placeholders})", kept_servers
        ).rowcount
    else:
        removed = conn.execute("DELETE FROM mcp_servers").rowcount
    cleared["mcp_servers"] = removed

    for index in FTS_INDEXES:
        if index in tables:
            conn.execute(f"INSERT INTO \"{index}\"(\"{index}\") VALUES('rebuild')")

    row = conn.execute("SELECT value FROM settings WHERE key = 'systemSettings'").fetchone()
    system_settings = json.loads(row[0]) if row and row[0] else {}
    system_settings["httpServerPort"] = port
    system_settings["httpServerExpose"] = False
    system_settings["preventSleepDuringAgentWork"] = False
    now_ms = int(time.time() * 1000)
    if row:
        conn.execute(
            "UPDATE settings SET value = ?, updated_at = ? WHERE key = 'systemSettings'",
            (json.dumps(system_settings), now_ms),
        )
    else:
        conn.execute(
            "INSERT INTO settings(key, value, created_at, updated_at) VALUES ('systemSettings', ?, ?, ?)",
            (json.dumps(system_settings), now_ms, now_ms),
        )
    conn.commit()

    return {
        "assistant_id": assistant_id,
        "port": port,
        "kept_mcp_server_ids": kept_servers,
        "cleared_rows": cleared,
    }


def build_seed(source_db: Path, layout: InstanceLayout, *, assistant_id: str, port: int) -> dict[str, object]:
    if not source_db.is_file():
        raise InstanceError(f"Source database not found: {source_db}")
    layout.seed_db.parent.mkdir(parents=True, exist_ok=True)
    if layout.seed_db.exists():
        layout.seed_db.unlink()

    source = sqlite3.connect(f"file:{source_db}?mode=ro", uri=True)
    try:
        dest = sqlite3.connect(layout.seed_db)
        try:
            source.backup(dest)
            summary = prune_seed(dest, assistant_id=assistant_id, port=port)
            dest.execute("VACUUM")
        finally:
            dest.close()
    finally:
        source.close()

    summary["source_db"] = str(source_db)
    summary["created_at_ms"] = int(time.time() * 1000)
    layout.seed_meta.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    return summary


# Settings rows that decide which model or endpoint a session calls. They follow the
# primary profile on every launch instead of staying frozen at seed time.
LLM_SETTING_KEYS = (
    "serviceConfigs",
    "customProviders",
    "preferredModel",
    "fallbackModel",
    "complexityModelMapping",
    "temperatureOverrideEnabled",
    "temperature",
)


def _json_or_none(raw: object) -> object:
    if not isinstance(raw, str):
        return None
    try:
        return json.loads(raw)
    except ValueError:
        return None


def sync_llm_settings(source_db: Path, run_db: Path) -> dict[str, object]:
    """Copy LLM settings rows from the primary DB (read-only) into the run copy.

    Rows the primary profile does not have are removed from the run copy, so the seed's
    old values never survive. Returns the effective settings with API keys reduced to
    ``has_api_key`` flags.
    """
    source = sqlite3.connect(f"file:{source_db}?mode=ro", uri=True)
    try:
        rows = {
            key: value
            for key, value in source.execute(
                f"SELECT key, value FROM settings WHERE key IN ({','.join('?' for _ in LLM_SETTING_KEYS)})",
                LLM_SETTING_KEYS,
            )
        }
    finally:
        source.close()

    now_ms = int(time.time() * 1000)
    dest = sqlite3.connect(run_db)
    try:
        for key in LLM_SETTING_KEYS:
            if key in rows:
                dest.execute(
                    "INSERT INTO settings(key, value, created_at, updated_at) VALUES (?, ?, ?, ?) "
                    "ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                    (key, rows[key], now_ms, now_ms),
                )
            else:
                dest.execute("DELETE FROM settings WHERE key = ?", (key,))
        dest.commit()
    finally:
        dest.close()

    return describe_llm_settings({key: _json_or_none(rows.get(key)) for key in LLM_SETTING_KEYS})


def describe_llm_settings(values: dict[str, object]) -> dict[str, object]:
    """Summarize LLM settings for run output. Never includes API keys."""

    def choice(value: object) -> str | None:
        if isinstance(value, dict) and value.get("provider") and value.get("model"):
            return f"{value['provider']}/{value['model']}"
        return None

    mapping = values.get("complexityModelMapping")
    complexity = {
        level: choice(mapping.get(level)) if isinstance(mapping, dict) else None
        for level in ("low", "normal", "high")
    }
    services = values.get("serviceConfigs")
    configured_services = {
        name: {"base_url": cfg.get("baseUrl") or None, "has_api_key": bool(cfg.get("apiKey"))}
        for name, cfg in (services.items() if isinstance(services, dict) else [])
        if isinstance(cfg, dict) and (cfg.get("apiKey") or cfg.get("baseUrl"))
    }
    custom = values.get("customProviders")
    custom_providers = [
        {
            "id": item.get("id"),
            "name": item.get("name"),
            "base_url": item.get("baseUrl"),
            "has_api_key": bool(item.get("apiKey")),
        }
        for item in (custom if isinstance(custom, list) else [])
        if isinstance(item, dict)
    ]
    preferred = choice(values.get("preferredModel"))
    fallback = choice(values.get("fallbackModel"))
    in_effect = sorted({m for m in [preferred, fallback, *complexity.values()] if m})
    return {
        "preferred_model": preferred,
        "fallback_model": fallback,
        "complexity_model_mapping": complexity,
        "models_in_effect": in_effect,
        "mixed_models": len(in_effect) > 1,
        "service_configs": configured_services,
        "custom_providers": custom_providers,
        "temperature_override_enabled": values.get("temperatureOverrideEnabled"),
        "temperature": values.get("temperature"),
    }


def _llm_source_db(layout: InstanceLayout) -> Path:
    """Primary DB the LLM settings follow: the seed's recorded source, else the default."""
    if layout.seed_meta.is_file():
        recorded = _json_or_none(layout.seed_meta.read_text(encoding="utf-8"))
        if isinstance(recorded, dict) and recorded.get("source_db"):
            return Path(str(recorded["source_db"]))
    return default_source_db()


def apply_prod_llm_settings(layout: InstanceLayout) -> dict[str, object]:
    """Refresh the run copy's LLM settings from the primary profile and record them."""
    source_db = _llm_source_db(layout)
    if not source_db.is_file():
        raise InstanceError(
            f"Primary database not found: {source_db}. LLM settings must follow the prod app; "
            "pass --source-db to 'seed' or start the prod app once."
        )
    summary = sync_llm_settings(source_db, layout.db_path)
    summary["source_db"] = str(source_db)
    layout.llm_settings_file.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    return summary


def _read_seed_port(layout: InstanceLayout) -> int:
    if not layout.seed_meta.is_file():
        raise InstanceError(f"No seed at {layout.seed_db}. Run the 'seed' command first.")
    return int(json.loads(layout.seed_meta.read_text(encoding="utf-8"))["port"])


def _ensure_not_primary_profile(layout: InstanceLayout) -> None:
    primary = default_source_db().parent.resolve()
    if layout.data_dir.resolve() == primary:
        raise InstanceError(f"Refusing to use the primary data dir as a bench instance: {primary}")


def reset_run_dir(layout: InstanceLayout) -> None:
    _ensure_not_primary_profile(layout)
    if layout.run_dir.exists():
        shutil.rmtree(layout.run_dir)
    layout.data_dir.mkdir(parents=True)
    for directory in (layout.home_dir, layout.tmp_dir, layout.app_dir):
        directory.mkdir()
    shutil.copy2(layout.seed_db, layout.db_path)


def sandbox_enabled(env: dict[str, str]) -> bool:
    return env.get(INSTANCE_SANDBOX_ENV, "").strip() == "1"


def sandbox_command(layout: InstanceLayout, app_bin: Path, env: dict[str, str]) -> list[str]:
    """bubblewrap argv that runs ``app_bin`` with only the run dir writable."""
    run_dir = str(layout.run_dir.resolve())
    command = [
        "bwrap",
        "--ro-bind", "/", "/",
        "--dev-bind", "/dev", "/dev",
        "--bind", run_dir, run_dir,
        "--bind", str(layout.tmp_dir.resolve()), "/tmp",
        "--ro-bind-try", "/tmp/.X11-unix", "/tmp/.X11-unix",
        "--bind", str(layout.app_dir.resolve()), SANDBOX_WORKDIR,
    ]  # fmt: skip
    runtime_dir = env.get("XDG_RUNTIME_DIR")
    if runtime_dir:
        # Wayland, D-Bus and WebKit sockets live here.
        command += ["--bind-try", runtime_dir, runtime_dir]
    return [*command, "--", str(app_bin.resolve())]


def _check_sandbox_available() -> None:
    if shutil.which("bwrap") is None:
        raise InstanceError("Host-mode sandbox needs bubblewrap (bwrap) on PATH.")
    if not Path(SANDBOX_WORKDIR).is_dir():
        raise InstanceError(
            f"Host-mode sandbox mounts its workspace over {SANDBOX_WORKDIR}, which must exist "
            f"(sudo mkdir {SANDBOX_WORKDIR})."
        )
    probe = subprocess.run(
        ["bwrap", "--ro-bind", "/", "/", "--", "true"], capture_output=True, text=True, check=False
    )
    if probe.returncode != 0:
        raise InstanceError(
            "bwrap cannot create a sandbox here "
            f"({probe.stderr.strip() or probe.returncode}). On Ubuntu, unprivileged user "
            "namespaces need an AppArmor profile for /usr/bin/bwrap."
        )


def instance_env(layout: InstanceLayout, base: dict[str, str] | None = None) -> dict[str, str]:
    env = dict(os.environ if base is None else base)
    if sandbox_enabled(env):
        real_home = env.get("HOME") or str(Path.home())
        # Docker CLI resolves credentials and contexts from $HOME/.docker.
        env.setdefault("DOCKER_CONFIG", str(Path(real_home) / ".docker"))
        env["HOME"] = str(layout.home_dir.resolve())
    env.update(
        {
            "LIBRAGENT_DATA_DIR": str(layout.data_dir.resolve()),
            "LIBRAGENT_DB_PATH": str(layout.db_path.resolve()),
            "LIBRAGENT_PROFILE": "prod",
            # Isolates WebView storage and any XDG-scoped app state as well.
            "XDG_DATA_HOME": str(layout.xdg_data.resolve()),
            "XDG_CACHE_HOME": str((layout.run_dir / "xdg-cache").resolve()),
            "XDG_CONFIG_HOME": str((layout.run_dir / "xdg-config").resolve()),
        }
    )
    return env


def _port_in_use(port: int) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        return sock.connect_ex(("127.0.0.1", port)) == 0


def _wait_healthy(port: int, timeout_sec: float, process: subprocess.Popen[bytes]) -> bool:
    deadline = time.monotonic() + timeout_sec
    url = f"http://127.0.0.1:{port}/api/health"
    while time.monotonic() < deadline:
        if process.poll() is not None:
            return False
        try:
            with urllib.request.urlopen(url, timeout=2) as response:
                if response.status == 200:
                    return True
        except (urllib.error.URLError, OSError):
            pass
        time.sleep(1)
    return False


def _read_pid(layout: InstanceLayout) -> int | None:
    try:
        return int(layout.pid_file.read_text(encoding="utf-8").strip())
    except (OSError, ValueError):
        return None


def _pid_alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def launch(layout: InstanceLayout, app_bin: Path, *, timeout_sec: float) -> int:
    port = _read_seed_port(layout)
    existing = _read_pid(layout)
    if existing is not None and _pid_alive(existing):
        raise InstanceError(f"Bench instance already running (pid {existing}). Run 'stop' first.")
    if _port_in_use(port):
        raise InstanceError(f"Port {port} is already in use; the bench instance would fall back to another port.")
    if not app_bin.is_file():
        raise InstanceError(f"App binary not found: {app_bin}")
    env = instance_env(layout)
    if sandbox_enabled(env):
        _check_sandbox_available()

    reset_run_dir(layout)
    llm = apply_prod_llm_settings(layout)
    command = sandbox_command(layout, app_bin, env) if sandbox_enabled(env) else [str(app_bin.resolve())]
    log = layout.log_file.open("wb")
    process = subprocess.Popen(
        command,
        env=env,
        stdout=log,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    layout.pid_file.write_text(str(process.pid), encoding="utf-8")

    if not _wait_healthy(port, timeout_sec, process):
        stop(layout)
        raise InstanceError(f"Bench instance did not become healthy on port {port}; see {layout.log_file}")
    safe_print(f"Bench instance ready: pid={process.pid} api_url=http://localhost:{port}/api")
    safe_print(f"Data dir: {layout.data_dir.resolve()}")
    safe_print(
        f"LLM settings from {llm['source_db']}: models={', '.join(llm['models_in_effect']) or 'none'}"  # type: ignore[arg-type]
        + (" (MIXED)" if llm["mixed_models"] else "")
    )
    return process.pid


def instance_pids(layout: InstanceLayout, proc_root: Path = Path("/proc")) -> list[int]:
    """PIDs that inherited this instance's data dir, including detached sidecars.

    The browser sidecar runs in its own process group, so killing the app's group
    leaves it (and its Chromium) writing into the run dir.
    """
    marker = f"LIBRAGENT_DATA_DIR={layout.data_dir.resolve()}".encode()
    pids: list[int] = []
    if not proc_root.is_dir():
        return pids
    own_pid = str(os.getpid())
    for entry in proc_root.iterdir():
        if not entry.name.isdigit() or entry.name == own_pid:
            continue
        try:
            environ = (entry / "environ").read_bytes()
        except OSError:
            continue
        if marker in environ.split(b"\0"):
            pids.append(int(entry.name))
    return pids


def _terminate(pids: list[int], grace_sec: float) -> None:
    for pid in pids:
        with contextlib.suppress(ProcessLookupError):
            os.kill(pid, signal.SIGTERM)
    deadline = time.monotonic() + grace_sec
    while time.monotonic() < deadline and any(_pid_alive(pid) for pid in pids):
        time.sleep(0.2)
    for pid in pids:
        if _pid_alive(pid):
            with contextlib.suppress(ProcessLookupError):
                os.kill(pid, signal.SIGKILL)


def _owns_pid(layout: InstanceLayout, pid: int, proc_root: Path = Path("/proc")) -> bool:
    """False when pid.pid is stale and its PID now belongs to an unrelated process.

    Without /proc (macOS) ownership cannot be checked, so the recorded PID is trusted.
    """
    if not proc_root.is_dir():
        return True
    return pid in instance_pids(layout, proc_root=proc_root)


def stop(layout: InstanceLayout, *, grace_sec: float = 10.0) -> None:
    pid = _read_pid(layout)
    if pid is not None and _pid_alive(pid) and _owns_pid(layout, pid):
        with contextlib.suppress(ProcessLookupError):
            os.killpg(pid, signal.SIGTERM)
        deadline = time.monotonic() + grace_sec
        while time.monotonic() < deadline and _pid_alive(pid):
            time.sleep(0.5)
        if _pid_alive(pid):
            with contextlib.suppress(ProcessLookupError):
                os.killpg(pid, signal.SIGKILL)
        safe_print(f"Stopped bench instance (pid {pid}).")
    else:
        safe_print("No running bench instance.")
    layout.pid_file.unlink(missing_ok=True)

    strays = instance_pids(layout)
    if strays:
        _terminate(strays, grace_sec)
        safe_print(f"Stopped {len(strays)} detached bench process(es): {strays}")


def relaunch(layout: InstanceLayout, app_bin: Path, *, timeout_sec: float = DEFAULT_LAUNCH_TIMEOUT_SEC) -> int:
    """Stop any running instance and start a new one on a fresh copy of the seed."""
    stop(layout)
    port = _read_seed_port(layout)
    deadline = time.monotonic() + 10.0
    while _port_in_use(port) and time.monotonic() < deadline:
        time.sleep(0.2)
    return launch(layout, app_bin, timeout_sec=timeout_sec)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--root", type=Path, default=DEFAULT_ROOT, help="Bench instance root directory")
    sub = parser.add_subparsers(dest="command", required=True)

    seed_cmd = sub.add_parser("seed", help="Build a pruned seed DB from the primary profile (read-only)")
    seed_cmd.add_argument("--assistant-id", required=True)
    seed_cmd.add_argument("--port", type=int, default=DEFAULT_PORT)
    seed_cmd.add_argument("--source-db", type=Path, default=None)

    launch_cmd = sub.add_parser("launch", help="Start the app on a fresh copy of the seed")
    launch_cmd.add_argument("--app-bin", type=Path, required=True)
    launch_cmd.add_argument("--timeout", type=float, default=DEFAULT_LAUNCH_TIMEOUT_SEC)

    stop_cmd = sub.add_parser("stop", help="Stop the running bench instance")
    stop_cmd.add_argument("--force", action="store_true", help="SIGKILL immediately instead of waiting for a graceful exit")

    args = parser.parse_args(argv)
    layout = InstanceLayout(args.root)

    try:
        if args.command == "seed":
            summary = build_seed(
                args.source_db or default_source_db(), layout, assistant_id=args.assistant_id, port=args.port
            )
            safe_print(json.dumps(summary, indent=2))
        elif args.command == "launch":
            launch(layout, args.app_bin, timeout_sec=args.timeout)
        elif args.command == "stop":
            stop(layout, grace_sec=0.0 if args.force else 10.0)
    except InstanceError as error:
        safe_print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
