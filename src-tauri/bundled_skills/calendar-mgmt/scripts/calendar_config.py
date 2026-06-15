#!/usr/bin/env python3
"""Shared config and Google Calendar credential helpers for calendar-mgmt."""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path
from typing import Any

CONFIG_DIR = Path.home() / ".libragent"
CONFIG_PATH = CONFIG_DIR / "calendar_config.json"

GOOGLE_SCOPES = ["https://www.googleapis.com/auth/calendar"]


def ensure_config_dir() -> None:
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)


def load_config() -> dict[str, Any]:
    if not CONFIG_PATH.exists():
        raise FileNotFoundError("calendar config not found")
    return json.loads(CONFIG_PATH.read_text(encoding="utf-8"))


def save_config(config: dict[str, Any]) -> None:
    ensure_config_dir()
    CONFIG_PATH.write_text(json.dumps(config, indent=2), encoding="utf-8")
    try:
        os.chmod(CONFIG_PATH, 0o600)
    except OSError:
        pass


def default_config() -> dict[str, Any]:
    return {"provider": "google", "google": {}, "caldav": None}


def resolve_secret(
    direct_value: str | None,
    env_name: str | None,
    secret_label: str,
) -> str:
    if direct_value:
        return direct_value.strip()
    if env_name:
        value = os.environ.get(env_name, "").strip()
        if value:
            return value
    raise ValueError(f"{secret_label} is required (use hidden prompt env var).")


def google_section_valid(google: dict[str, Any] | None) -> tuple[bool, str | None]:
    if not google:
        return False, "google section is missing"
    if not google.get("client_id"):
        return False, "google.client_id is missing"
    if not google.get("client_secret"):
        return False, "google.client_secret is missing"
    tokens = google.get("tokens")
    if not isinstance(tokens, dict) or not tokens.get("refresh_token"):
        return False, "google OAuth tokens are missing or incomplete"
    return True, None


def get_google_credentials():
    """Return refreshed google.oauth2.credentials.Credentials."""
    try:
        from google.auth.transport.requests import Request
        from google.oauth2.credentials import Credentials
    except ImportError as exc:
        raise ImportError(
            "google-auth is not installed. Run: pip install google-auth google-auth-oauthlib google-api-python-client"
        ) from exc

    config = load_config()
    if config.get("provider") != "google":
        raise ValueError(f"Unsupported provider: {config.get('provider')!r}")

    google = config.get("google") or {}
    ok, reason = google_section_valid(google)
    if not ok:
        raise ValueError(reason or "invalid google config")

    creds = Credentials.from_authorized_user_info(google["tokens"], GOOGLE_SCOPES)
    if creds.expired and creds.refresh_token:
        creds.refresh(Request())
        google["tokens"] = json.loads(creds.to_json())
        config["google"] = google
        save_config(config)
    return creds


def build_calendar_service():
    try:
        from googleapiclient.discovery import build
    except ImportError as exc:
        raise ImportError(
            "google-api-python-client is not installed. Run: pip install google-api-python-client"
        ) from exc

    creds = get_google_credentials()
    return build("calendar", "v3", credentials=creds, cache_discovery=False)


def json_error(message: str, code: int = 1) -> None:
    print(json.dumps({"error": message}), file=sys.stderr)
    sys.exit(code)
