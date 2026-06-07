#!/usr/bin/env python3
"""
Check LibrAgent X config and session file existence.

Exit codes:
  0 — Config exists and session file exists
  1 — Config or session does not exist (first-time setup needed)
  2 — Config exists but is invalid / corrupt (reset needed)

Prints a JSON status object to stdout.
"""

import json
import sys
from pathlib import Path

CONFIG_PATH = Path.home() / ".libragent" / "x_config.json"
COOKIES_PATH = Path.home() / ".libragent" / "x_cookies.json"

def main() -> int:
    if not CONFIG_PATH.exists() or not COOKIES_PATH.exists():
        print(
            json.dumps(
                {
                    "status": "missing",
                    "message": "No X configuration or cookies found. Setup is required.",
                    "action": "setup",
                }
            )
        )
        return 1

    try:
        cfg = json.loads(CONFIG_PATH.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError) as e:
        print(
            json.dumps(
                {
                    "status": "corrupt",
                    "message": f"X config file is unreadable: {e}",
                    "action": "reset",
                }
            )
        )
        return 2

    if not cfg.get("username") or not cfg.get("email"):
        print(
            json.dumps(
                {
                    "status": "incomplete",
                    "message": "X config is missing username or email.",
                    "action": "reset",
                }
            )
        )
        return 2

    try:
        cookies_text = COOKIES_PATH.read_text(encoding="utf-8").strip()
        if not cookies_text:
            raise ValueError("Cookies file is empty")
        cookies_data = json.loads(cookies_text)
        if not cookies_data:
            raise ValueError("Cookies JSON data is empty or invalid")
    except (json.JSONDecodeError, OSError, ValueError) as e:
        print(
            json.dumps(
                {
                    "status": "corrupt",
                    "message": f"X cookies file is unreadable or empty: {e}",
                    "action": "reset",
                }
            )
        )
        return 2

    print(
        json.dumps(
            {
                "status": "ok",
                "username": cfg["username"],
                "message": "X configuration and session are valid.",
            }
        )
    )
    return 0

if __name__ == "__main__":
    sys.exit(main())
