"""Checks for scripts/run-harbor-bench.sh secret handling (#2510)."""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from typing import ClassVar

import pytest

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "run-harbor-bench.sh"


def test_session_api_key_never_reaches_a_python_argv() -> None:
    """argv is readable by every local user (ps, /proc/<pid>/cmdline)."""
    offenders = [
        line.strip()
        for line in SCRIPT.read_text(encoding="utf-8").splitlines()
        if re.search(r'"\$PYTHON"[^#]*\$\{?SESSION_API_KEY', line)
    ]
    assert offenders == []


class _RecordingHandler(BaseHTTPRequestHandler):
    seen: ClassVar[list[tuple[str, str | None]]] = []

    def do_GET(self) -> None:
        type(self).seen.append((self.path, self.headers.get("Authorization")))
        if self.path == "/api/assistants":
            body: object = [{"id": "a-1", "name": "Coding Assistant"}]
        else:
            body = {"provider": "openai", "model": "gpt-x"}
        data = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *_args: object) -> None:
        pass


@pytest.mark.skipif(shutil.which("bash") is None, reason="needs bash")
@pytest.mark.skipif(sys.platform == "win32", reason="bash script")
def test_lookups_send_the_key_from_the_environment() -> None:
    _RecordingHandler.seen = []
    server = HTTPServer(("127.0.0.1", 0), _RecordingHandler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        source = SCRIPT.read_text(encoding="utf-8")
        functions = "\n".join(
            re.search(  # type: ignore[union-attr]
                rf"^{name}\(\) \{{\n.*?^\}}$", source, re.DOTALL | re.MULTILINE
            ).group(0)
            for name in ("resolve_assistant_id", "resolve_harbor_model")
        )
        result = subprocess.run(
            [
                "bash",
                "-c",
                functions + "\nresolve_assistant_id; resolve_harbor_model",
            ],
            env={
                "PATH": "/usr/bin:/bin",
                "PYTHON": sys.executable,
                "API_URL": f"http://127.0.0.1:{server.server_port}/api",
                "ASSISTANT_NAME": "Coding Assistant",
                "ASSISTANT_ID": "",
                "HARBOR_MODEL": "",
                "SESSION_API_KEY": "k-secret",
            },
            capture_output=True,
            text=True,
            timeout=60,
            check=True,
        )
    finally:
        server.shutdown()

    assert result.stdout.split() == ["a-1", "openai/gpt-x"]
    assert _RecordingHandler.seen == [
        ("/api/assistants", "Bearer k-secret"),
        ("/api/settings/preferredModel", "Bearer k-secret"),
    ]


MATRIX_DIR = (
    Path(__file__).resolve().parent / "libragent-session-e2e-matrix"
)


@pytest.mark.skipif(shutil.which("bash") is None, reason="needs bash")
@pytest.mark.skipif(shutil.which("curl") is None, reason="needs curl")
@pytest.mark.skipif(sys.platform == "win32", reason="bash script")
@pytest.mark.skipif(
    sys.platform not in ("linux", "darwin"),
    reason="host OS token is derived from uname on POSIX",
)
def test_path_preset_auto_excludes_windows_only_tasks() -> None:
    """#2655: e2e-b11-no-runshell requires a Windows host, so a Linux/macOS
    runner must add `-x e2e-b11-no-runshell` instead of running a trial that
    can only fail."""
    _RecordingHandler.seen = []
    server = HTTPServer(("127.0.0.1", 0), _RecordingHandler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        result = subprocess.run(
            [
                "bash",
                str(SCRIPT),
                "--dry-run",
                "--skip-health-check",
                "--preset",
                "path",
                "--path",
                str(MATRIX_DIR),
                "--api-url",
                f"http://127.0.0.1:{server.server_port}/api",
            ],
            env={**os.environ, "SESSION_API_KEY": "k-secret"},
            capture_output=True,
            text=True,
            timeout=120,
            check=True,
        )
    finally:
        server.shutdown()

    combined = result.stdout + result.stderr
    assert "requires_host_os=windows" in combined
    assert "-x e2e-b11-no-runshell" in combined
