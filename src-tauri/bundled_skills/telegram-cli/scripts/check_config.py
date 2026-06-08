#!/usr/bin/env python3
"""
Check LibrAgent Telegram config, session file, and authorization state.

Exit codes:
  0 — Config exists, session file exists, and user is authorized
  1 — Config/session missing or user is not authorized (setup needed)
  2 — Config exists but is invalid / corrupt (reset needed)

Prints a JSON status object to stdout.
"""

import json
import sys
from pathlib import Path

CONFIG_PATH = Path.home() / ".libragent" / "telegram_config.json"
REQUIRED_FIELDS = {"api_id", "api_hash", "phone", "session_name"}


def get_session_base_path(session_name: str) -> Path:
    """Return Telethon session base path (without .session suffix)."""
    return Path.home() / ".libragent" / session_name


def check_authorization(cfg: dict) -> tuple[bool, str | None]:
    """
    Verify the Telethon session is authorized.

    Returns (authorized, error_message).
    """
    try:
        from telethon import TelegramClient
    except ImportError:
        return False, "telethon is not installed. Run: pip3 install telethon"

    session_path = get_session_base_path(cfg["session_name"])
    client = TelegramClient(str(session_path), int(cfg["api_id"]), cfg["api_hash"])

    try:
        client.loop.run_until_complete(client.connect())
        authorized = client.loop.run_until_complete(client.is_user_authorized())
        return authorized, None
    except Exception as exc:
        return False, f"Failed to verify Telegram session: {exc}"
    finally:
        disconnect = client.disconnect()
        if disconnect is not None:
            client.loop.run_until_complete(disconnect)


def main() -> int:
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
