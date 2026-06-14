#!/usr/bin/env python3
"""
Validate LibrAgent calendar config.

Exit codes:
  0 — Config exists and Google OAuth tokens are present
  1 — Config does not exist (first-time setup needed)
  2 — Config exists but is invalid / corrupt / needs re-auth

Prints a JSON status object to stdout.
"""

from __future__ import annotations

import json
import sys

from calendar_config import CONFIG_PATH, google_section_valid, load_config


def main() -> int:
    if not CONFIG_PATH.exists():
        print(
            json.dumps(
                {
                    "status": "missing",
                    "message": "No calendar config found. Run the skill setup flow (Google OAuth).",
                    "action": "setup",
                }
            )
        )
        return 1

    try:
        config = load_config()
    except (json.JSONDecodeError, OSError) as exc:
        print(
            json.dumps(
                {
                    "status": "corrupt",
                    "message": f"Config file is unreadable or corrupt: {exc}",
                    "action": "reset",
                }
            )
        )
        return 2

    provider = config.get("provider")
    if provider != "google":
        print(
            json.dumps(
                {
                    "status": "unsupported",
                    "message": f"Provider {provider!r} is not supported in this MVP build.",
                    "action": "setup",
                }
            )
        )
        return 2

    google = config.get("google")
    ok, reason = google_section_valid(google if isinstance(google, dict) else None)
    if not ok:
        print(
            json.dumps(
                {
                    "status": "incomplete",
                    "message": reason,
                    "action": "setup" if reason and "client_id" in reason else "oauth",
                }
            )
        )
        return 2

    try:
        from calendar_config import get_google_credentials

        get_google_credentials()
    except ImportError as exc:
        print(
            json.dumps(
                {
                    "status": "dependency_missing",
                    "message": str(exc),
                    "action": "install_deps",
                }
            )
        )
        return 2
    except Exception as exc:
        print(
            json.dumps(
                {
                    "status": "auth_invalid",
                    "message": f"Stored Google tokens could not be refreshed: {exc}",
                    "action": "oauth",
                }
            )
        )
        return 2

    print(
        json.dumps(
            {
                "status": "ok",
                "provider": "google",
                "message": "Google Calendar config is valid.",
            }
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
