#!/usr/bin/env python3
"""
Check LibrAgent Telegram config and session file existence.

Exit codes:
  0 — Config exists and session file exists
  1 — Config or session does not exist (first-time setup needed)
  2 — Config exists but is invalid / corrupt (reset needed)

Prints a JSON status object to stdout.
"""

import json
import sys
from pathlib import Path

CONFIG_PATH = Path.home() / ".libragent" / "telegram_config.json"
REQUIRED_FIELDS = {"api_id", "api_hash", "phone", "session_name"}


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
    session_file = Path.home() / ".libragent" / f"{cfg['session_name']}.session"
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

    # --- All good ---
    print(
        json.dumps(
            {
                "status": "ok",
                "phone": cfg["phone"],
                "session_name": cfg["session_name"],
                "message": "Config and session are valid.",
            }
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
