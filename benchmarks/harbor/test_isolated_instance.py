from __future__ import annotations

import json
import sqlite3
from pathlib import Path

import pytest

from benchmarks.harbor.isolated_instance import (
    InstanceError,
    InstanceLayout,
    _owns_pid,
    apply_prod_llm_settings,
    build_seed,
    instance_pids,
    instance_env,
    reset_run_dir,
    safe_print,
    sandbox_command,
    stop,
)

ASSISTANT_ID = "bench-assistant"


def _make_source_db(path: Path) -> None:
    conn = sqlite3.connect(path)
    conn.executescript(
        """
        CREATE TABLE assistants (id TEXT PRIMARY KEY, name TEXT, config TEXT);
        CREATE TABLE mcp_servers (id TEXT PRIMARY KEY, name TEXT);
        CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT, created_at INTEGER, updated_at INTEGER);
        CREATE TABLE sessions (id TEXT PRIMARY KEY);
        CREATE TABLE messages (id TEXT PRIMARY KEY, session_id TEXT REFERENCES sessions(id));
        CREATE TABLE scheduled_tasks (id TEXT PRIMARY KEY);
        CREATE TABLE knowledge_chunks_v2 (id INTEGER PRIMARY KEY AUTOINCREMENT, content TEXT, tags TEXT, source TEXT);
        CREATE VIRTUAL TABLE knowledge_chunks_fts USING fts5(
            content, tags, source, content='knowledge_chunks_v2', content_rowid='id'
        );
        CREATE TRIGGER knowledge_chunks_fts_ad AFTER DELETE ON knowledge_chunks_v2 BEGIN
            INSERT INTO knowledge_chunks_fts(knowledge_chunks_fts, rowid, content, tags, source)
            VALUES ('delete', old.id, old.content, old.tags, old.source);
        END;
        """
    )
    conn.execute(
        "INSERT INTO assistants VALUES (?, 'Bench', ?)",
        (ASSISTANT_ID, json.dumps({"mcpServerIds": ["keep-1"]})),
    )
    conn.executemany(
        "INSERT INTO mcp_servers VALUES (?, ?)",
        [("keep-1", "exa"), ("drop-1", "memory"), ("drop-2", "telegram")],
    )
    conn.execute(
        "INSERT INTO settings VALUES ('systemSettings', ?, 0, 0)",
        (json.dumps({"httpServerPort": 3030, "httpServerExpose": True, "other": 1}),),
    )
    conn.execute("INSERT INTO sessions VALUES ('s1')")
    conn.execute("INSERT INTO messages VALUES ('m1', 's1')")
    conn.execute("INSERT INTO scheduled_tasks VALUES ('t1')")
    conn.execute("INSERT INTO knowledge_chunks_v2(content, tags, source) VALUES ('AURORA-7 deploy', '', 'x')")
    conn.execute("INSERT INTO knowledge_chunks_fts(knowledge_chunks_fts) VALUES('rebuild')")
    conn.commit()
    conn.close()


@pytest.fixture
def seeded(tmp_path: Path) -> tuple[Path, InstanceLayout, dict[str, object]]:
    source = tmp_path / "source.db"
    _make_source_db(source)
    layout = InstanceLayout(tmp_path / "bench")
    summary = build_seed(source, layout, assistant_id=ASSISTANT_ID, port=3131)
    return source, layout, summary


def test_seed_keeps_only_assistant_servers_and_clears_runtime_state(seeded) -> None:
    _, layout, summary = seeded
    conn = sqlite3.connect(layout.seed_db)
    assert [r[0] for r in conn.execute("SELECT id FROM mcp_servers")] == ["keep-1"]
    for table in ("sessions", "messages", "scheduled_tasks", "knowledge_chunks_v2"):
        assert conn.execute(f"SELECT count(*) FROM {table}").fetchone()[0] == 0
    assert conn.execute("SELECT count(*) FROM knowledge_chunks_fts WHERE knowledge_chunks_fts MATCH 'AURORA'").fetchone()[0] == 0
    assert conn.execute("SELECT count(*) FROM assistants").fetchone()[0] == 1
    settings = json.loads(conn.execute("SELECT value FROM settings WHERE key='systemSettings'").fetchone()[0])
    assert settings["httpServerPort"] == 3131
    assert settings["httpServerExpose"] is False
    assert settings["other"] == 1
    assert summary["kept_mcp_server_ids"] == ["keep-1"]


def test_seed_never_mutates_source(seeded) -> None:
    source, _, _ = seeded
    conn = sqlite3.connect(source)
    assert conn.execute("SELECT count(*) FROM mcp_servers").fetchone()[0] == 3
    assert conn.execute("SELECT count(*) FROM sessions").fetchone()[0] == 1


def test_seed_rejects_unknown_assistant(tmp_path: Path) -> None:
    source = tmp_path / "source.db"
    _make_source_db(source)
    with pytest.raises(InstanceError):
        build_seed(source, InstanceLayout(tmp_path / "bench"), assistant_id="missing", port=3131)


def test_reset_run_dir_starts_from_pristine_seed(seeded) -> None:
    _, layout, _ = seeded
    reset_run_dir(layout)
    conn = sqlite3.connect(layout.db_path)
    conn.execute("INSERT INTO mcp_servers VALUES ('leaked', 'memory')")
    conn.commit()
    conn.close()
    (layout.data_dir / "user_skills" / "leaked").mkdir(parents=True)

    reset_run_dir(layout)

    conn = sqlite3.connect(layout.db_path)
    assert [r[0] for r in conn.execute("SELECT id FROM mcp_servers")] == ["keep-1"]
    assert not (layout.data_dir / "user_skills").exists()


def test_instance_env_isolates_data_and_xdg_dirs(tmp_path: Path) -> None:
    layout = InstanceLayout(tmp_path / "bench")
    env = instance_env(layout, base={"HOME": "/home/u", "DOCKER_HOST": "unix:///x"})
    assert env["LIBRAGENT_DATA_DIR"].endswith("run/xdg-data/com.fritzprix.libragent")
    assert env["LIBRAGENT_DB_PATH"].endswith("com.fritzprix.libragent/libragent_v2.db")
    assert env["XDG_DATA_HOME"].endswith("run/xdg-data")
    assert env["HOME"] == "/home/u"
    assert env["DOCKER_HOST"] == "unix:///x"
    assert "DOCKER_CONFIG" not in env


def test_instance_env_sandboxes_home_but_keeps_docker_config(tmp_path: Path) -> None:
    layout = InstanceLayout(tmp_path / "bench")
    env = instance_env(
        layout, base={"HOME": "/home/u", "LIBRAGENT_BENCH_INSTANCE_SANDBOX": "1"}
    )
    assert env["HOME"] == str(layout.home_dir.resolve())
    assert env["DOCKER_CONFIG"] == "/home/u/.docker"

    explicit = instance_env(
        layout,
        base={
            "HOME": "/home/u",
            "DOCKER_CONFIG": "/etc/docker-cfg",
            "LIBRAGENT_BENCH_INSTANCE_SANDBOX": "1",
        },
    )
    assert explicit["DOCKER_CONFIG"] == "/etc/docker-cfg"


def test_sandbox_command_keeps_only_run_dir_writable(tmp_path: Path) -> None:
    layout = InstanceLayout(tmp_path / "bench")
    run_dir = str(layout.run_dir.resolve())
    command = sandbox_command(layout, Path("/opt/libragent"), {"XDG_RUNTIME_DIR": "/run/user/1000"})

    assert command[0] == "bwrap"
    assert "--unshare-pid" in command
    assert ("--proc", "/proc") in list(zip(command, command[1:]))
    assert "--die-with-parent" not in command  # the instance outlives `launch`
    assert command[-2:] == ["--", "/opt/libragent"]
    pairs = list(zip(command, command[1:], command[2:]))
    assert ("--bind", run_dir, run_dir) in pairs
    assert ("--bind", str(layout.tmp_dir.resolve()), "/tmp") in pairs
    assert ("--bind", str(layout.app_dir.resolve()), "/app") in pairs
    assert ("--bind-try", "/run/user/1000", "/run/user/1000") in pairs
    writable = [dest for flag, _, dest in pairs if flag in {"--bind", "--bind-try"}]
    assert writable == [run_dir, "/tmp", "/app", "/run/user/1000"]


def test_instance_pids_matches_inherited_data_dir(tmp_path: Path) -> None:
    layout = InstanceLayout(tmp_path / "bench")
    proc = tmp_path / "proc"
    marker = f"LIBRAGENT_DATA_DIR={layout.data_dir.resolve()}"
    for pid, environ in {
        "101": f"PATH=/bin\0{marker}\0",
        "102": f"LIBRAGENT_DATA_DIR={layout.data_dir.resolve()}/nested\0",
        "103": "PATH=/bin\0",
    }.items():
        (proc / pid).mkdir(parents=True)
        (proc / pid / "environ").write_bytes(environ.encode())
    (proc / "self").mkdir()

    assert instance_pids(layout, proc_root=proc) == [101]


def test_owns_pid_rejects_reused_pid(tmp_path: Path) -> None:
    layout = InstanceLayout(tmp_path / "bench")
    proc = tmp_path / "proc"
    marker = f"LIBRAGENT_DATA_DIR={layout.data_dir.resolve()}"
    for pid, environ in {"201": f"{marker}\0", "202": "PATH=/bin\0"}.items():
        (proc / pid).mkdir(parents=True)
        (proc / pid / "environ").write_bytes(environ.encode())

    assert _owns_pid(layout, 201, proc_root=proc)
    assert not _owns_pid(layout, 202, proc_root=proc)


def test_owns_pid_trusts_pid_file_without_proc(tmp_path: Path) -> None:
    layout = InstanceLayout(tmp_path / "bench")

    assert _owns_pid(layout, 1, proc_root=tmp_path / "missing-proc")


def _set_setting(db: Path, key: str, value: object) -> None:
    conn = sqlite3.connect(db)
    conn.execute(
        "INSERT INTO settings VALUES (?, ?, 0, 0) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (key, json.dumps(value)),
    )
    conn.commit()
    conn.close()


def _llm_rows(db: Path) -> dict[str, object]:
    conn = sqlite3.connect(db)
    rows = {k: json.loads(v) for k, v in conn.execute("SELECT key, value FROM settings")}
    conn.close()
    return rows


def test_launch_llm_settings_follow_current_prod_and_replace_seed_values(seeded) -> None:
    source, layout, _ = seeded
    # Seed time: one model, plus a mapping that the prod app later drops.
    _set_setting(source, "preferredModel", {"provider": "openai", "model": "old"})
    _set_setting(source, "complexityModelMapping", {"high": {"provider": "gemini", "model": "flash"}})
    build_seed(source, layout, assistant_id=ASSISTANT_ID, port=3131)

    # Prod changes afterwards.
    _set_setting(source, "preferredModel", {"provider": "openai", "model": "qwen"})
    _set_setting(source, "serviceConfigs", {"openai": {"apiKey": "sk-secret", "baseUrl": "http://x/v1"}})
    conn = sqlite3.connect(source)
    conn.execute("DELETE FROM settings WHERE key = 'complexityModelMapping'")
    conn.commit()
    conn.close()

    reset_run_dir(layout)
    summary = apply_prod_llm_settings(layout)

    rows = _llm_rows(layout.db_path)
    assert rows["preferredModel"] == {"provider": "openai", "model": "qwen"}
    assert "complexityModelMapping" not in rows
    assert rows["serviceConfigs"]["openai"]["apiKey"] == "sk-secret"
    assert rows["systemSettings"]["httpServerPort"] == 3131  # isolation untouched
    assert summary["models_in_effect"] == ["openai/qwen"]
    assert summary["mixed_models"] is False


def test_llm_settings_record_flags_mixed_models_and_hides_api_keys(seeded) -> None:
    source, layout, _ = seeded
    _set_setting(source, "preferredModel", {"provider": "openai", "model": "qwen"})
    _set_setting(source, "complexityModelMapping", {"high": {"provider": "gemini", "model": "flash"}})
    _set_setting(source, "serviceConfigs", {"openai": {"apiKey": "sk-secret", "baseUrl": "http://x/v1"}})
    _set_setting(source, "customProviders", [{"id": "c1", "name": "vllm", "baseUrl": "http://v/v1", "apiKey": "tok-secret"}])
    reset_run_dir(layout)

    summary = apply_prod_llm_settings(layout)

    assert summary["mixed_models"] is True
    assert summary["models_in_effect"] == ["gemini/flash", "openai/qwen"]
    assert summary["service_configs"] == {"openai": {"base_url": "http://x/v1", "has_api_key": True}}
    recorded = layout.llm_settings_file.read_text(encoding="utf-8")
    assert "sk-secret" not in recorded and "tok-secret" not in recorded
    assert json.loads(recorded)["custom_providers"][0]["has_api_key"] is True


def test_llm_settings_sync_never_mutates_source_and_requires_it(seeded, tmp_path: Path) -> None:
    source, layout, _ = seeded
    _set_setting(source, "preferredModel", {"provider": "openai", "model": "qwen"})
    before = _llm_rows(source)
    reset_run_dir(layout)
    apply_prod_llm_settings(layout)
    assert _llm_rows(source) == before

    source.unlink()
    with pytest.raises(InstanceError):
        apply_prod_llm_settings(layout)


class _BrokenStream:
    """Stands in for stdout after the controlling terminal went away."""

    def write(self, _text: str) -> int:
        raise OSError(5, "Input/output error")

    def flush(self) -> None:
        raise OSError(5, "Input/output error")


def test_safe_print_swallows_broken_stdout(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr("sys.stdout", _BrokenStream())
    safe_print("diagnostic")  # must not raise


def test_stop_completes_when_stdout_is_broken(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr("sys.stdout", _BrokenStream())
    layout = InstanceLayout(tmp_path / "bench")
    layout.root.mkdir(parents=True)
    stop(layout)  # no running instance: used to die on print("No running bench instance.")
    assert not layout.pid_file.exists()
