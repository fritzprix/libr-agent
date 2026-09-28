#!/usr/bin/env python3
"""
Check LibrAgent Instagram config and session file existence.

Exit codes:
  0 — Config exists and session file exists
  1 — Config or session does not exist (first-time setup needed)
  2 — Config exists but is invalid / corrupt (reset needed)

Prints a JSON status object to stdout.
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

CONFIG_PATH = Path.home() / ".libragent" / "ig_config.json"
SESSION_PATH = Path.home() / ".libragent" / "ig_session.json"


def harden_private_file(path: Path) -> None:
    """Restrict credential files to owner read/write only (best-effort)."""
    try:
        os.chmod(path, 0o600)
    except OSError:
        pass


def main() -> int:
    if not CONFIG_PATH.exists() or not SESSION_PATH.exists():
        print(
            json.dumps(
                {
                    "status": "missing",
                    "message": "No Instagram configuration or session found. Setup is required.",
                    "action": "setup",
                }
            )
        )
        return 1

    try:
        cfg = json.loads(CONFIG_PATH.read_text(encoding="utf-8-sig"))
    except (json.JSONDecodeError, OSError) as e:
        print(
            json.dumps(
                {
                    "status": "corrupt",
                    "message": f"Instagram config file is unreadable: {e}",
                    "action": "reset",
                }
            )
        )
        return 2

    if not cfg.get("username"):
        print(
            json.dumps(
                {
                    "status": "incomplete",
                    "message": "Instagram config is missing username.",
                    "action": "reset",
                }
            )
        )
        return 2

    try:
        session_text = SESSION_PATH.read_text(encoding="utf-8-sig").strip()
        if not session_text:
            raise ValueError("Session file is empty")
        session_data = json.loads(session_text)
        if not session_data:
            raise ValueError("Session JSON data is empty or invalid")
    except (json.JSONDecodeError, OSError, ValueError) as e:
        print(
            json.dumps(
                {
                    "status": "corrupt",
                    "message": f"Instagram session file is unreadable or empty: {e}",
                    "action": "reset",
                }
            )
        )
        return 2

    harden_private_file(CONFIG_PATH)
    harden_private_file(SESSION_PATH)

    print(
        json.dumps(
            {
                "status": "ok",
                "username": cfg["username"],
                "message": "Instagram configuration and session are valid.",
            }
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
