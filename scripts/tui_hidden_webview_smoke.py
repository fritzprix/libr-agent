#!/usr/bin/env python3
"""TUI-hosted LLM round-trip smoke for `libragent --tui` (issue #2784).

What this run proves today
--------------------------
Isolated **TUI standalone bring-up**: `libragent-tui` hosts a scratch backend,
signals frontend readiness, and completes one Session-API turn with a tool
call + result + final reply. That is bring-up evidence only (owner note on
#2784) — not full hidden-WebView / GUI coexistence proof.

What full #2784 evidence still requires
---------------------------------------
1. Force the WebView LLM path (`LIBRAGENT_LLM_EXECUTOR=webview` is set here)
   and confirm logs show the turn on the frontend, not
   `Rust LLM executor: running completion`.
2. Start the **prod GUI first**, then attach TUI on the **same profile**
   (B-02/B-03) — no second HTTP server, no port fallback, no killing a
   pre-existing `libragent` by name. That path is blocked until coexistence
   fixes land; this script stays on an isolated scratch profile on purpose.

How it stays isolated
---------------------
SQLite is copied (WAL-safe) into a scratch dir; the TUI uses
`LIBRAGENT_DB_PATH` + `LIBRAGENT_DATA_DIR` there. The Session API is open
(no injected key). This harness never stops processes it did not start.

Usage
-----
    python scripts/tui_hidden_webview_smoke.py [--binary PATH] [--source-db PATH]
        [--timeout SECS] [--keep] [--evidence FILE]

Exit code 0 = pass, 1 = fail (reason printed; the TUI log tail is included).
Requires a configured provider in the source DB (same as the GUI).
"""

import argparse
import json
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
PROMPT = (
    "You MUST call the `scratchpad__listNote` tool exactly once now, with "
    "empty arguments, before writing any answer. After the tool result "
    "arrives, reply with exactly: SMOKE-DONE"
)
DESCRIPTOR_TIMEOUT = 180.0
TURN_TIMEOUT_DEFAULT = 420.0


def fail(msg: str, log_tail: str = "") -> None:
    print(f"FAIL: {msg}")
    if log_tail:
        print("--- TUI log tail ---")
        print(log_tail)
    sys.exit(1)


DEV_URL = "http://localhost:1420"


def default_binary() -> Path:
    env_bin = os.environ.get("LIBRAGENT_TUI_BIN")
    if env_bin:
        return Path(env_bin)
    name = "libragent-tui.exe" if os.name == "nt" else "libragent-tui"
    # Release first: it embeds the built frontend and works with no dev
    # server. A debug binary loads the Vite devUrl instead (see preflight).
    for profile_dir in ("release", "debug"):
        candidate = REPO_ROOT / "src-tauri" / "target" / profile_dir / name
        if candidate.is_file():
            return candidate
    fail("libragent-tui binary not found (build it or pass --binary)")
    raise SystemExit  # unreachable


def preflight_binary(binary: Path) -> None:
    """A debug Tauri binary loads the frontend from the Vite devUrl, not from
    the embedded dist. With no dev server on that port the hidden WebView is
    blank, nobody answers `llm:completion-request`, and the session just
    stays busy forever — so catch it up front."""
    if "debug" not in binary.parts and "debug" not in binary.name.lower():
        return
    try:
        with urllib.request.urlopen(DEV_URL, timeout=2) as resp:
            if resp.status < 500:
                print(f"note: debug binary; Vite dev server answers at {DEV_URL}")
                return
    except OSError:
        pass
    fail(
        f"{binary} is a debug build and loads the frontend from the Vite "
        f"devUrl ({DEV_URL}), which is not reachable. The hidden WebView "
        "would load nothing and no LLM turn could complete. Either start "
        "`pnpm dev` in another shell, or build the release binary: "
        "`pnpm build && node scripts/run-rust-command.cjs build --release "
        "--bin libragent-tui`"
    )


def default_data_dir() -> Path:
    if os.name == "nt":
        return Path(os.environ["APPDATA"]) / "com.fritzprix.libragent"
    if sys.platform == "darwin":
        return Path.home() / "Library" / "Application Support" / "com.fritzprix.libragent"
    return Path.home() / ".local" / "share" / "com.fritzprix.libragent"


def default_source_db() -> Path:
    env_db = os.environ.get("LIBRAGENT_SOURCE_DB")
    if env_db:
        return Path(env_db)
    data_dir = default_data_dir()
    # The prod DB carries the provider the GUI actually runs on — the same
    # configuration the acceptance criteria point at. The dev DB is a small
    # fallback for developer machines.
    for name in ("libragent_v2.db", "libragent_v2.dev.db"):
        candidate = data_dir / name
        if candidate.is_file():
            return candidate
    fail(f"no source DB found under {data_dir} (pass --source-db)")
    raise SystemExit  # unreachable


def copy_db(src: Path, dst: Path) -> None:
    """WAL-safe copy via the SQLite backup API (the GUI may be running)."""
    src_conn = sqlite3.connect(f"file:{src}?mode=ro", uri=True)
    dst_conn = sqlite3.connect(dst)
    try:
        src_conn.backup(dst_conn)
        # The scratch backend must never try to recover the live user's
        # in-flight sessions from the copy: neutralize them first.
        dst_conn.execute(
            "update sessions set status = 'paused' "
            "where status in ('busy', 'queued')"
        )
        dst_conn.commit()
    finally:
        dst_conn.close()
        src_conn.close()


def assert_provider_configured(db_path: Path) -> str:
    conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    try:
        row = conn.execute(
            "select value from settings where key = 'serviceConfigs'"
        ).fetchone()
    finally:
        conn.close()
    if not row:
        fail("scratch DB has no serviceConfigs — source DB is unconfigured")
    configs = json.loads(row[0])
    configured = [
        name
        for name, cfg in configs.items()
        if name not in ("empty",) and (cfg.get("apiKey") or cfg.get("baseUrl"))
    ]
    if not configured:
        fail("no provider with an apiKey/baseUrl in the source DB — configure one first")
    return ",".join(sorted(configured))


def pick_default_provider_model(db_path: Path) -> tuple[str, str]:
    """Prefer a first-party Gemini/OpenAI path over a custom OpenAI-compat relay.

    The GUI preferredModel can point at a custom relay that hangs under the
    hidden WebView; live evidence for #2784 used Gemini on macOS/Linux.
    """
    conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    try:
        configs_row = conn.execute(
            "select value from settings where key = 'serviceConfigs'"
        ).fetchone()
        preferred_row = conn.execute(
            "select value from settings where key = 'preferredModel'"
        ).fetchone()
    finally:
        conn.close()
    configs = json.loads(configs_row[0]) if configs_row else {}

    def usable(name: str) -> bool:
        cfg = configs.get(name) or {}
        return bool(cfg.get("apiKey") or cfg.get("baseUrl"))

    if preferred_row:
        preferred = json.loads(preferred_row[0])
        provider = preferred.get("provider") or ""
        model = preferred.get("model") or ""
        if provider and model and not provider.startswith("custom:") and usable(provider):
            return provider, model

    if usable("gemini"):
        return "gemini", "gemini-flash-latest"
    if usable("openai"):
        return "openai", "gpt-4o-mini"
    fail("no first-party gemini/openai provider configured for the smoke")
    raise SystemExit  # unreachable


def spawn_tui(binary: Path, scratch: Path, log_path: Path):
    env = os.environ.copy()
    env["LIBRAGENT_DB_PATH"] = str(scratch / "smoke.db")
    env["LIBRAGENT_DATA_DIR"] = str(scratch / "data")
    # Isolate the WebView runtime data folder, mirroring headless/mod.rs.
    # A running GUI owns the default EBWebView folder; a second process
    # sharing it never runs page JS, so "frontend-ready" never arrives
    # (observed on Windows with the GUI live, #2784).
    webview_dir = scratch / "webview-cache"
    webview_dir.mkdir(parents=True, exist_ok=True)
    if os.name == "nt":
        env["WEBVIEW2_USER_DATA_FOLDER"] = str(webview_dir)
    elif sys.platform == "linux":
        env["XDG_CACHE_HOME"] = str(webview_dir)
    # Force the phase-1 frontend LLM path (#2760 / #2804 otherwise route
    # gemini/openai to Rust and the smoke would not touch the WebView).
    env["LIBRAGENT_LLM_EXECUTOR"] = "webview"
    # Guarantee an open Session API on the scratch backend (no injected key).
    env.pop("LIBRAGENT_SESSION_API_KEY", None)
    env.pop("LIBRAGENT_SESSION_API_KEY_FILE", None)
    log_file = open(log_path, "wb")
    if os.name == "nt":
        # Do not SW_HIDE / CREATE_NEW_CONSOLE / kill other libragent PIDs:
        # WebView2 needs a normal process host; windows stay non-interactive
        # via the TUI show_windows=false / off-screen config.
        proc = subprocess.Popen(
            [str(binary), "--profile", "prod"],
            cwd=str(scratch),
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=log_file,
            stderr=subprocess.STDOUT,
        )
        closer = None
    else:
        import pty

        master, slave = pty.openpty()
        proc = subprocess.Popen(
            [str(binary), "--profile", "prod"],
            cwd=str(scratch),
            env=env,
            stdin=slave,
            stdout=slave,
            stderr=slave,
            close_fds=True,
        )
        os.close(slave)

        def pump():
            try:
                while True:
                    chunk = os.read(master, 65536)
                    if not chunk:
                        break
                    log_file.write(chunk)
                    log_file.flush()
            except OSError:
                pass

        closer = (master, pump)
    return proc, log_file, closer


def wait_for_descriptor(scratch: Path, timeout: float) -> int:
    deadline = time.time() + timeout
    while time.time() < deadline:
        for record in scratch.glob("smoke.db.backend.*.json"):
            try:
                with open(record, encoding="utf-8") as fh:
                    return json.load(fh)["port"]
            except (OSError, KeyError, ValueError):
                pass
        time.sleep(0.5)
    fail(f"backend descriptor not published within {timeout:.0f}s")
    raise SystemExit  # unreachable


def http_json(port: int, method: str, path: str, body=None):
    url = f"http://127.0.0.1:{port}{path}"
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method)
    if data:
        req.add_header("Content-Type", "application/json")
    with urllib.request.urlopen(req, timeout=15) as resp:
        raw = resp.read().decode()
    return json.loads(raw) if raw else {}


def wait_for_health(port: int, timeout: float) -> None:
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            health = http_json(port, "GET", "/api/health")
            if health.get("ready", True):
                return
        except (urllib.error.URLError, ConnectionError, ValueError):
            pass
        time.sleep(1.0)
    fail(f"backend not ready within {timeout:.0f}s")


def tauri_app_log_path() -> Path | None:
    """Tauri `app_log_dir` (not LIBRAGENT_DATA_DIR) — where setup_file_logger writes."""
    if os.name == "nt":
        local = os.environ.get("LOCALAPPDATA")
        if local:
            return Path(local) / "com.fritzprix.libragent" / "logs" / "libragent.log"
    if sys.platform == "darwin":
        return (
            Path.home()
            / "Library"
            / "Logs"
            / "com.fritzprix.libragent"
            / "libragent.log"
        )
    xdg = os.environ.get("XDG_DATA_HOME")
    base = Path(xdg) if xdg else Path.home() / ".local" / "share"
    return base / "com.fritzprix.libragent" / "logs" / "libragent.log"


def wait_for_frontend_ready(log_path: Path, timeout: float) -> None:
    """Session API calls before this hang; completions find no WebView listener.

    macOS/Linux live notes for #2784: wait for \"Frontend signaled readiness\".
    On Windows the smoke redirects stdout, so also watch the Tauri file log.
    Ignore pre-rotation leftovers from a previous process in the shared app log.
    """
    deadline = time.time() + timeout
    needle = "Frontend signaled readiness"
    app_log = tauri_app_log_path()
    before_size = 0
    if app_log is not None and app_log.is_file():
        try:
            before_size = app_log.stat().st_size
        except OSError:
            before_size = 0
    fresh_app_log = False
    while time.time() < deadline:
        try:
            pipe_text = log_path.read_text(errors="replace")
        except OSError:
            pipe_text = ""
        if needle in pipe_text:
            print("frontend ready")
            return
        if app_log is not None:
            try:
                size = app_log.stat().st_size
                app_text = app_log.read_text(errors="replace")
            except OSError:
                size = 0
                app_text = ""
            if size < before_size or (
                "File logger initialized" in app_text and size != before_size
            ):
                fresh_app_log = True
            if fresh_app_log and needle in app_text:
                print("frontend ready")
                return
        time.sleep(0.5)
    fail(
        f"frontend never signaled readiness within {timeout:.0f}s — if this is "
        "a cargo-built release binary, it may have been compiled without the "
        "`custom-protocol` feature and therefore loads the Vite devUrl "
        "(http://localhost:1420) instead of the embedded frontend. Build with "
        "`pnpm tauri build --no-bundle --bin libragent-tui`, or "
        "`node scripts/run-rust-command.cjs build --release --bin "
        "libragent-tui --features tauri/custom-protocol`."
    )


def sse_collector(port: int, session_id: str, events: list, stop: threading.Event) -> None:
    try:
        resp = urllib.request.urlopen(
            f"http://127.0.0.1:{port}/api/sessions/{session_id}/events", timeout=60
        )
        current = None
        for raw in resp:
            if stop.is_set():
                break
            line = raw.decode(errors="replace").strip()
            if line.startswith("event:"):
                current = line.split(":", 1)[1].strip()
            elif line.startswith("data:") and current:
                events.append(current)
    except OSError:
        pass  # the stream closing early must not fail the smoke


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--source-db", type=Path)
    parser.add_argument("--provider", help="override session provider (e.g. gemini)")
    parser.add_argument("--model", help="override session model")
    parser.add_argument("--timeout", type=float, default=TURN_TIMEOUT_DEFAULT)
    parser.add_argument("--keep", action="store_true", help="keep the scratch dir")
    parser.add_argument("--evidence", type=Path, help="write evidence JSON here")
    args = parser.parse_args()

    binary = args.binary or default_binary()
    source_db = args.source_db or default_source_db()
    if not binary.is_file():
        fail(f"binary not found: {binary}")
    preflight_binary(binary)
    if not source_db.is_file():
        fail(f"source DB not found: {source_db}")

    scratch = Path(tempfile.mkdtemp(prefix="libragent-tui-smoke-"))
    log_path = scratch / "tui.log"
    print(f"scratch: {scratch}")
    print(f"binary:  {binary}")
    print(f"db:      {source_db}")

    copy_db(source_db, scratch / "smoke.db")
    providers = assert_provider_configured(scratch / "smoke.db")
    print(f"providers configured in scratch DB: {providers}")
    if args.provider and args.model:
        provider, model = args.provider, args.model
    else:
        provider, model = pick_default_provider_model(scratch / "smoke.db")
    print(f"session provider/model: {provider} / {model}")

    proc, log_file, closer = spawn_tui(binary, scratch, log_path)

    def tail(limit: int = 40) -> str:
        log_file.flush()
        try:
            lines = log_path.read_text(errors="replace").splitlines()
        except OSError:
            return ""
        return "\n".join(lines[-limit:])

    try:
        port = wait_for_descriptor(scratch, DESCRIPTOR_TIMEOUT)
        print(f"backend port: {port}")
        wait_for_health(port, DESCRIPTOR_TIMEOUT)
        wait_for_frontend_ready(log_path, DESCRIPTOR_TIMEOUT)

        assistants = http_json(port, "GET", "/api/assistants")
        items = assistants if isinstance(assistants, list) else assistants.get("assistants", [])
        if not items:
            fail("no assistants in scratch DB", tail())
        assistant_id = items[0].get("id") or items[0].get("assistantId")

        session = http_json(
            port,
            "POST",
            "/api/sessions",
            {
                "assistantId": assistant_id,
                "executionMode": "yolo",
                "name": "tui-hidden-webview-smoke",
                "provider": provider,
                "model": model,
            },
        )
        session_id = session["id"]
        print(f"session: {session_id} (status {session.get('status')})")

        events: list = []
        stop = threading.Event()
        sse_thread = threading.Thread(
            target=sse_collector, args=(port, session_id, events, stop), daemon=True
        )
        sse_thread.start()
        time.sleep(0.5)

        sent = time.time()
        http_json(port, "POST", f"/api/sessions/{session_id}/messages", {"content": PROMPT})

        final_status = None
        meta: dict = {}
        deadline = sent + args.timeout
        while time.time() < deadline:
            snapshot = http_json(port, "GET", f"/api/sessions/{session_id}")
            meta = snapshot.get("session", snapshot)
            final_status = meta.get("status")
            if final_status in ("idle", "error", "paused"):
                break
            time.sleep(2.0)
        turn_secs = time.time() - sent
        if final_status is None or final_status == "busy":
            fail(
                f"turn still {final_status or 'unknown'} after {turn_secs:.0f}s "
                f"(hidden WebView likely did not answer)",
                tail(),
            )
        if final_status == "error":
            fail("session ended in error", tail())
        if final_status == "paused":
            fail("session paused (unapproved tool call?) — yolo approval did not apply", tail())

        messages = http_json(port, "GET", f"/api/sessions/{session_id}/messages")
        if isinstance(messages, dict):
            messages = messages.get("messages", [])
        user_msgs = [m for m in messages if m.get("role") == "user"]
        tool_calls_msgs = [m for m in messages if m.get("role") == "assistant" and m.get("toolCalls")]
        tool_results = [m for m in messages if m.get("role") == "tool"]
        call_ids = {
            call.get("id")
            for m in tool_calls_msgs
            for call in (m.get("toolCalls") or [])
        }
        matched_results = [m for m in tool_results if m.get("toolCallId") in call_ids]
        final_texts = [
            m
            for m in messages
            if m.get("role") == "assistant"
            and not m.get("toolCalls")
            and any(
                part.get("type") == "text" and (part.get("text") or "").strip()
                for part in (m.get("content") or [])
            )
        ]
        if not any(m.get("created_at", 0) >= sent * 1000 or m.get("createdAt", 0) >= sent * 1000 for m in final_texts):
            # created_at may be seconds; be generous: any final assistant text is fine
            pass

        problems = []
        if not tool_calls_msgs:
            problems.append("no assistant message with toolCalls (tool call did not happen)")
        if not matched_results:
            problems.append("no tool result message matching an issued tool call id")
        if not final_texts:
            problems.append("no final assistant text message (reply did not land)")
        if not user_msgs:
            problems.append("user prompt was not persisted")

        call_names = sorted(
            {
                call.get("function", {}).get("name", "?")
                for m in tool_calls_msgs
                for call in (m.get("toolCalls") or [])
            }
        )
        # Prefer the fresh Tauri app log (stdout pipe is often empty on Windows).
        app_log = tauri_app_log_path()
        try:
            combined_log = (app_log.read_text(errors="replace") if app_log else "") + "\n" + tail(200)
        except OSError:
            combined_log = tail(200)
        rust_hits = combined_log.count("Rust LLM executor: running completion")
        webview_hits = combined_log.count("[webview]")
        if rust_hits:
            problems.append(
                f"Rust LLM executor handled the turn ({rust_hits} hit(s)); "
                "expected LIBRAGENT_LLM_EXECUTOR=webview"
            )
        if webview_hits == 0:
            problems.append("no [webview] log lines — frontend LLM path not observed")

        evidence = {
            "issue": 2784,
            "evidenceKind": "tui-standalone-bring-up",
            "llmExecutor": "webview",
            "coexistence": False,
            "platform": sys.platform,
            "binary": str(binary),
            "sourceDb": str(source_db),
            "providersConfigured": providers,
            "sessionId": session_id,
            "sessionProvider": meta.get("provider"),
            "sessionModel": meta.get("model"),
            "turnSeconds": round(turn_secs, 1),
            "finalStatus": final_status,
            "rustExecutorCompletions": rust_hits,
            "webviewLogLines": webview_hits,
            "userMessages": len(user_msgs),
            "toolCallMessages": len(tool_calls_msgs),
            "toolCallNames": call_names,
            "toolResultMessages": len(matched_results),
            "finalAssistantTextMessages": len(final_texts),
            "sseMessageEvents": sum(1 for e in events if e == "message"),
            "sseStatusEvents": sum(1 for e in events if e == "status"),
            "passed": not problems,
            "problems": problems,
        }
        if args.evidence:
            args.evidence.write_text(
                json.dumps(evidence, indent=2), encoding="utf-8"
            )
        print(json.dumps(evidence, indent=2))
        if problems:
            fail("; ".join(problems), tail())
        print("PASS: hidden WebView answered an LLM turn (tool call + result + final reply persisted)")
    finally:
        stop = locals().get("stop")
        if isinstance(stop, threading.Event):
            stop.set()
        proc.terminate()
        try:
            proc.wait(timeout=20)
        except subprocess.TimeoutExpired:
            proc.kill()
        log_file.close()
        if closer:
            try:
                os.close(closer[0])
            except OSError:
                pass
        if not args.keep:
            shutil.rmtree(scratch, ignore_errors=True)
        else:
            print(f"scratch kept: {scratch}")


if __name__ == "__main__":
    main()
