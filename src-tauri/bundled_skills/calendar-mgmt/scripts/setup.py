#!/usr/bin/env python3
"""
LibrAgent Google Calendar setup.

Stores OAuth client credentials and runs a local loopback OAuth flow.
Secrets must come from hidden shell prompts (env vars), never from chat.

Usage:
  python setup.py --client-id "xxx.apps.googleusercontent.com" --client-secret-env LIBRAGENT_GOOGLE_CLIENT_SECRET --oauth
  python setup.py --reset --client-id "..." --client-secret-env LIBRAGENT_GOOGLE_CLIENT_SECRET --oauth
  python setup.py --oauth   # re-run OAuth when client credentials already saved
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from calendar_config import (
    CONFIG_PATH,
    GOOGLE_SCOPES,
    default_config,
    load_config,
    resolve_secret,
    save_config,
)


def run_google_oauth(client_id: str, client_secret: str) -> dict:
    try:
        from google_auth_oauthlib.flow import InstalledAppFlow
    except ImportError:
        print(
            json.dumps(
                {
                    "status": "error",
                    "message": "google-auth-oauthlib is not installed. Run: pip install google-auth-oauthlib google-auth google-api-python-client",
                }
            ),
            file=sys.stderr,
        )
        sys.exit(1)

    client_config = {
        "installed": {
            "client_id": client_id,
            "client_secret": client_secret,
            "auth_uri": "https://accounts.google.com/o/oauth2/auth",
            "token_uri": "https://oauth2.googleapis.com/token",
            "redirect_uris": ["http://localhost"],
        }
    }

    flow = InstalledAppFlow.from_client_config(client_config, GOOGLE_SCOPES)
    creds = flow.run_local_server(port=0, prompt="consent", open_browser=True)
    return json.loads(creds.to_json())


def main() -> int:
    parser = argparse.ArgumentParser(description="LibrAgent Google Calendar setup")
    parser.add_argument("--client-id", help="Google OAuth Desktop client ID")
    parser.add_argument(
        "--client-secret-env",
        default="LIBRAGENT_GOOGLE_CLIENT_SECRET",
        help="Environment variable containing the OAuth client secret",
    )
    parser.add_argument(
        "--oauth",
        action="store_true",
        help="Run browser OAuth flow and store refresh token",
    )
    parser.add_argument(
        "--reset",
        action="store_true",
        help="Delete existing config before setup",
    )
    args = parser.parse_args()

    if args.reset and CONFIG_PATH.exists():
        CONFIG_PATH.unlink()

    config = default_config()
    if CONFIG_PATH.exists():
        try:
            config = load_config()
        except json.JSONDecodeError:
            config = default_config()

    google = config.get("google") if isinstance(config.get("google"), dict) else {}

    client_id = (args.client_id or google.get("client_id") or "").strip()
    if not client_id and args.oauth:
        print(
            json.dumps(
                {
                    "status": "error",
                    "message": "client_id is required for OAuth setup.",
                }
            ),
            file=sys.stderr,
        )
        return 1

    if args.client_id:
        google["client_id"] = client_id

    client_secret = (google.get("client_secret") or "").strip()
    if args.client_secret_env:
        try:
            client_secret = resolve_secret(
                None,
                args.client_secret_env,
                "Google OAuth client secret",
            )
            google["client_secret"] = client_secret
        except ValueError as exc:
            if not client_secret:
                print(json.dumps({"status": "error", "message": str(exc)}), file=sys.stderr)
                return 1

    config["provider"] = "google"
    config["google"] = google
    save_config(config)

    if not args.oauth:
        print(
            json.dumps(
                {
                    "status": "partial",
                    "message": "Client credentials saved. Run again with --oauth to complete browser authorization.",
                    "next": f'python "{Path(__file__).name}" --oauth',
                }
            )
        )
        return 0

    if not google.get("client_secret"):
        print(
            json.dumps(
                {
                    "status": "error",
                    "message": "client_secret missing. Pass --client-secret-env after a hidden password prompt.",
                }
            ),
            file=sys.stderr,
        )
        return 1

    try:
        tokens = run_google_oauth(client_id, google["client_secret"])
    except Exception as exc:
        print(
            json.dumps(
                {
                    "status": "error",
                    "message": f"OAuth flow failed: {exc}",
                }
            ),
            file=sys.stderr,
        )
        return 1

    google["tokens"] = tokens
    config["google"] = google
    save_config(config)

    print(
        json.dumps(
            {
                "status": "ok",
                "message": "Google Calendar connected. Tokens saved to ~/.libragent/calendar_config.json",
                "provider": "google",
            }
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
