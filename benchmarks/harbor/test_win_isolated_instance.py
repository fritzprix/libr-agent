"""Windows-path tests for the isolated bench instance (#2905).

These run on any OS: the Win32 primitives in win_process are stubbed, so what
gets tested is the manager's Windows logic - path isolation, the launch
manifest (PID + creation time + job token), and stop dispatch - not ctypes.
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from benchmarks.harbor import isolated_instance
from benchmarks.harbor.isolated_instance import (
    InstanceLayout,
    _job_name,
    _owns_pid,
    _stop_windows,
    _write_instance_meta,
    default_source_db,
    instance_env,
    instance_pids,
    instance_running,
    read_instance_meta,
)


@pytest.fixture
def win(monkeypatch: pytest.MonkeyPatch) -> object:
    """Pretend to be Windows with a stubbed win_process module."""
    state = {
        "alive": True,
        "created": 111,
        "terminated": [],
        "taskkill": [],
    }
    monkeypatch.setattr(isolated_instance, "IS_WINDOWS", True)
    monkeypatch.setattr(isolated_instance.win_process, "pid_alive", lambda pid: state["alive"])
    monkeypatch.setattr(
        isolated_instance.win_process,
        "process_creation_time",
        lambda pid: state["created"],
    )

    def fake_terminate(name: str) -> bool:
        state["terminated"].append(name)
        return True

    monkeypatch.setattr(isolated_instance.win_process, "terminate_job_by_name", fake_terminate)

    def fake_run(command: list[str], **kwargs: object) -> None:
        state["taskkill"].append(command)

        class _Done:
            returncode = 0

        return _Done()

    monkeypatch.setattr(isolated_instance.subprocess, "run", fake_run)
    return state


def _layout(tmp_path: Path) -> InstanceLayout:
    return InstanceLayout(tmp_path / "bench")


def _write_manifest(layout: InstanceLayout, *, pid: int, created: int, job_name: str) -> None:
    layout.root.mkdir(parents=True, exist_ok=True)
    layout.pid_file.write_text(str(pid), encoding="utf-8")
    _write_instance_meta(layout, pid=pid, created=created, job_name=job_name)


def test_default_source_db_uses_appdata_on_windows(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(isolated_instance, "IS_WINDOWS", True)
    monkeypatch.setenv("APPDATA", str(tmp_path / "Roaming"))
    db = default_source_db()
    assert db == tmp_path / "Roaming" / "com.fritzprix.libragent" / "libragent_v2.db"


def test_instance_env_redirects_windows_profile_dirs(tmp_path: Path, win: object) -> None:
    layout = _layout(tmp_path)
    layout.run_dir.mkdir(parents=True)
    env = instance_env(layout, base={})
    assert env["APPDATA"] == str(layout.xdg_data.resolve())
    assert env["LOCALAPPDATA"] == str((layout.run_dir / "local-appdata").resolve())
    assert env["USERPROFILE"] == str(layout.home_dir.resolve())
    assert env["HOME"] == str(layout.home_dir.resolve())
    assert env["TEMP"] == str(layout.tmp_dir.resolve())
    # The app's own isolation vars stay in place too.
    assert env["LIBRAGENT_DATA_DIR"] == str(layout.data_dir.resolve())
    assert env["LIBRAGENT_PROFILE"] == "prod"


def test_manifest_roundtrip(tmp_path: Path, win: object) -> None:
    layout = _layout(tmp_path)
    layout.root.mkdir(parents=True)
    _write_instance_meta(layout, pid=4242, created=99, job_name="Local\\libragent-bench-aa")
    meta = read_instance_meta(layout)
    assert meta is not None
    assert meta["pid"] == 4242
    assert meta["created"] == 99
    assert _job_name(layout) == "Local\\libragent-bench-aa"


def test_manifest_absent_reads_none(tmp_path: Path, win: object) -> None:
    layout = _layout(tmp_path)
    layout.root.mkdir(parents=True)
    assert read_instance_meta(layout) is None
    assert _job_name(layout) is None


def test_instance_running_rejects_recycled_pid(tmp_path: Path, win: object) -> None:
    layout = _layout(tmp_path)
    _write_manifest(layout, pid=1234, created=111, job_name="Local\\libragent-bench-01")
    assert instance_running(layout) is True
    # Same PID, different creation time: the process is not ours.
    win["created"] = 222  # type: ignore[index]
    assert instance_running(layout) is False
    win["created"] = 111  # type: ignore[index]
    win["alive"] = False  # type: ignore[index]
    assert instance_running(layout) is False


def test_owns_pid_uses_creation_time_guard(tmp_path: Path, win: object) -> None:
    layout = _layout(tmp_path)
    _write_manifest(layout, pid=7, created=111, job_name="Local\\libragent-bench-02")
    assert _owns_pid(layout, 7) is True
    win["created"] = 222  # type: ignore[index]
    assert _owns_pid(layout, 7) is False


def test_stop_windows_terminates_job_tree(tmp_path: Path, win: object) -> None:
    layout = _layout(tmp_path)
    _write_manifest(layout, pid=99, created=111, job_name="Local\\libragent-bench-03")
    _stop_windows(layout, grace_sec=0.0)
    assert win["terminated"] == ["Local\\libragent-bench-03"]  # type: ignore[index]
    # Job termination succeeded, so the taskkill fallback must not fire.
    assert win["taskkill"] == []  # type: ignore[index]
    assert not layout.pid_file.exists()
    assert not layout.instance_meta.exists()


def test_stop_windows_falls_back_to_taskkill(
    tmp_path: Path, win: object, monkeypatch: pytest.MonkeyPatch
) -> None:
    layout = _layout(tmp_path)
    _write_manifest(layout, pid=99, created=111, job_name="Local\\libragent-bench-04")
    monkeypatch.setattr(isolated_instance.win_process, "terminate_job_by_name", lambda name: False)
    _stop_windows(layout, grace_sec=0.0)
    assert [["taskkill", "/F", "/T", "/PID", "99"]] == win["taskkill"]  # type: ignore[index]


def test_stop_windows_no_instance_is_noop(tmp_path: Path, win: object) -> None:
    layout = _layout(tmp_path)
    layout.root.mkdir(parents=True)
    _stop_windows(layout, grace_sec=0.0)
    assert win["terminated"] == []  # type: ignore[index]
    assert win["taskkill"] == []  # type: ignore[index]


def test_instance_pids_empty_on_windows(tmp_path: Path, win: object) -> None:
    layout = _layout(tmp_path)
    layout.run_dir.mkdir(parents=True)
    assert instance_pids(layout) == []


def test_sandbox_refused_on_windows(tmp_path: Path, win: object) -> None:
    with pytest.raises(isolated_instance.InstanceError, match="Linux-only"):
        isolated_instance._check_sandbox_available()


def test_json_manifest_malformed_is_none(tmp_path: Path, win: object) -> None:
    layout = _layout(tmp_path)
    layout.root.mkdir(parents=True)
    layout.instance_meta.write_text("not json", encoding="utf-8")
    assert read_instance_meta(layout) is None
