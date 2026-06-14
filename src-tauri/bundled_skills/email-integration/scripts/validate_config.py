#!/usr/bin/env python3
"""
Validate LibrAgent email config.

Exit codes:
  0 — Config exists and is structurally valid
  1 — Config does not exist (first-time setup needed)
  2 — Config exists but is invalid / corrupt (reset needed)

Prints a JSON status object to stdout.
"""

import json
import sys
from pathlib import Path

CONFIG_PATH = Path.home() / ".libragent" / "email_config"

REQUIRED_FIELDS = {"email", "password", "imap_host", "imap_port", "smtp_host", "smtp_port"}


def main() -> None:
    # --- Check existence ---
    if not CONFIG_PATH.exists():
        print(json.dumps({
            "status": "missing",
            "message": "No config found. Run the skill's Step 2 non-interactive setup flow to configure email, server fields, and password.",
            "action": "setup",
        }))
        sys.exit(1)

    # --- Check readability and JSON validity ---
    try:
        cfg = json.loads(CONFIG_PATH.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError) as e:
        print(json.dumps({
            "status": "corrupt",
            "message": f"Config file is unreadable or corrupt: {e}",
            "action": "reset",
        }))
        sys.exit(2)

    # --- Check required fields ---
    missing = REQUIRED_FIELDS - set(cfg.keys())
    if missing:
        print(json.dumps({
            "status": "incomplete",
            "message": f"Config is missing required fields: {sorted(missing)}",
            "action": "reset",
        }))
        sys.exit(2)

    # --- Check non-empty critical values ---
    for field in ("email", "password", "imap_host", "smtp_host"):
        if not cfg.get(field):
            print(json.dumps({
                "status": "incomplete",
                "message": f"Config field '{field}' is empty.",
                "action": "reset",
            }))
            sys.exit(2)

    # --- Check port numbers are integers ---
    for port_field in ("imap_port", "smtp_port"):
        if not isinstance(cfg.get(port_field), int):
            print(json.dumps({
                "status": "invalid",
                "message": f"Config field '{port_field}' must be an integer.",
                "action": "reset",
            }))
            sys.exit(2)

    # --- All good ---
    print(json.dumps({
        "status": "ok",
        "email": cfg["email"],
        "imap": f"{cfg['imap_host']}:{cfg['imap_port']}",
        "smtp": f"{cfg['smtp_host']}:{cfg['smtp_port']}",
        "message": "Config is valid.",
    }))
    sys.exit(0)


if __name__ == "__main__":
    main()
