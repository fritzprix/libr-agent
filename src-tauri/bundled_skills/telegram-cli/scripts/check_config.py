#!/usr/bin/env python3
"""
Check LibrAgent Telegram config, session file, and authorization state.

Exit codes:
  0 — Config exists, session file exists, and user is authorized
  1 — Config/session missing or user is not authorized (setup needed)
  2 — Config exists but is invalid / corrupt (reset needed)

Prints a JSON status object to stdout.
"""

import asyncio
import json
import os
import sys
from pathlib import Path

CONFIG_PATH = Path.home() / ".libragent" / "telegram_config.json"
OP_TIMEOUT_SEC = 60.0
DISCONNECT_TIMEOUT_SEC = 10.0
REQUIRED_FIELDS = {"api_id", "api_hash", "phone", "session_name"}


def configure_stdio_utf8() -> None:
    """Force UTF-8 on stdout/stderr to avoid cp949/mojibake issues across platforms."""
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is not None:
            try:
                reconfigure(encoding="utf-8", errors="replace")
            except (OSError, ValueError):
                pass


def get_session_base_path(session_name: str) -> Path:
    """Return Telethon session base path (without .session suffix)."""
    return Path.home() / ".libragent" / session_name


def harden_private_file(path: Path) -> None:
    """Restrict credential files to owner read/write only (best-effort)."""
    try:
        os.chmod(path, 0o600)
    except OSError:
        pass


def harden_session_files(session_base: Path) -> None:
    """Apply 0o600 to Telethon session artifacts if they exist."""
    for suffix in (".session", ".session-journal"):
        path = Path(str(session_base) + suffix)
        if path.exists():
            harden_private_file(path)


def check_authorization(cfg: dict) -> tuple[bool, str | None]:
    """
    Verify the Telethon session is authorized.

    Returns (authorized, error_message).
    """
    try:
        from telethon import TelegramClient
        from telethon.errors import AuthRestartError
    except ImportError:
        return False, "telethon is not installed. Run: python -m pip install telethon"

    session_path = get_session_base_path(cfg["session_name"])
    client = TelegramClient(str(session_path), int(cfg["api_id"]), cfg["api_hash"])

    try:
        client.loop.run_until_complete(asyncio.wait_for(client.connect(), timeout=OP_TIMEOUT_SEC))
        harden_session_files(session_path)
        harden_private_file(CONFIG_PATH)
        authorized = client.loop.run_until_complete(
            asyncio.wait_for(client.is_user_authorized(), timeout=OP_TIMEOUT_SEC)
        )
        return authorized, None
    except AuthRestartError:
        return False, "auth_restart_needed"
    except asyncio.TimeoutError:
        return False, f"Timed out verifying Telegram session after {int(OP_TIMEOUT_SEC)}s"
    except Exception as exc:
        return False, f"Failed to verify Telegram session: {exc}"
    finally:
        try:
            disconnect = client.disconnect()
            if disconnect is not None:
                client.loop.run_until_complete(
                    asyncio.wait_for(disconnect, timeout=DISCONNECT_TIMEOUT_SEC)
                )
        except Exception:
            pass


def main() -> int:
    configure_stdio_utf8()
    # --- Check config file existence ---
    if not CONFIG_PATH.exists():
        print(
            json.dumps(
                {
                    "status": "missing",
                    "message": "No Telegram config found. Run the skill's Step 2 setup flow to configure API credentials and phone.",
                    "action": "setup",
                }
            )
        )
        return 1

    # --- Check readability and JSON validity ---
    try:
        cfg = json.loads(CONFIG_PATH.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError) as e:
        print(
            json.dumps(
                {
                    "status": "corrupt",
                    "message": f"Config file is unreadable or corrupt: {e}",
                    "action": "reset",
                }
            )
        )
        return 2

    # --- Check required fields ---
    missing = REQUIRED_FIELDS - set(cfg.keys())
    if missing:
        print(
            json.dumps(
                {
                    "status": "incomplete",
                    "message": f"Config is missing required fields: {sorted(missing)}",
                    "action": "reset",
                }
            )
        )
        return 2

    # --- Check non-empty critical values ---
    for field in ("api_id", "api_hash", "phone", "session_name"):
        if not cfg.get(field):
            print(
                json.dumps(
                    {
                        "status": "incomplete",
                        "message": f"Config field '{field}' is empty.",
                        "action": "reset",
                    }
                )
            )
            return 2

    # --- Check session file existence ---
    session_file = get_session_base_path(cfg["session_name"]).with_suffix(".session")
    if not session_file.exists():
        print(
            json.dumps(
                {
                    "status": "missing_session",
                    "message": "Config exists but session file is missing. Re-run setup to authenticate.",
                    "action": "setup",
                }
            )
        )
        return 1

    # --- Verify Telethon authorization ---
    authorized, auth_error = check_authorization(cfg)
    if auth_error == "auth_restart_needed":
        print(
            json.dumps(
                {
                    "status": "auth_restart_needed",
                    "message": (
                        "Telegram requires restarting authentication. "
                        "Delete ~/.libragent/telegram_session.session and run send_code again."
                    ),
                    "action": "reset",
                }
            )
        )
        return 1

    if auth_error:
        print(
            json.dumps(
                {
                    "status": "error",
                    "message": auth_error,
                    "action": "setup",
                }
            )
        )
        return 1

    if not authorized:
        print(
            json.dumps(
                {
                    "status": "unauthorized",
                    "message": "Session file exists but is not authorized. Complete Step 2 sign_in (verification code and 2FA if needed).",
                    "action": "setup",
                }
            )
        )
        return 1

    print(
        json.dumps(
            {
                "status": "ok",
                "phone": cfg["phone"],
                "session_name": cfg["session_name"],
                "message": "Config, session, and authorization are valid.",
            }
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
