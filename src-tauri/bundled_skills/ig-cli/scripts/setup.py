#!/usr/bin/env python3
"""Setup Instagram authentication via instagrapi and persist a private session."""

from __future__ import annotations

import argparse
import json
import os
import sys
from concurrent.futures import ThreadPoolExecutor
from concurrent.futures import TimeoutError as FuturesTimeout
from pathlib import Path

try:
    from instagrapi import Client
    from instagrapi.exceptions import BadPassword, ClientError, TwoFactorRequired
except ImportError:
    print(
        json.dumps(
            {
                "status": "error",
                "message": "instagrapi is not installed. Run: python -m pip install instagrapi",
            }
        ),
        file=sys.stderr,
    )
    sys.exit(1)

CONFIG_DIR = Path.home() / ".libragent"
CONFIG_PATH = CONFIG_DIR / "ig_config.json"
SESSION_PATH = CONFIG_DIR / "ig_session.json"
# Legacy path from early prototypes — never write; remove if present.
LEGACY_PASSWORD_TMP = CONFIG_DIR / "ig_password.tmp"
OP_TIMEOUT_SEC = 60


def run_sync(fn, *args, timeout: float = OP_TIMEOUT_SEC, **kwargs):
    """Run a blocking instagrapi call with a hard timeout."""
    with ThreadPoolExecutor(max_workers=1) as pool:
        future = pool.submit(fn, *args, **kwargs)
        try:
            return future.result(timeout=timeout)
        except FuturesTimeout as exc:
            raise TimeoutError(f"Instagram operation timed out after {int(timeout)}s") from exc


def harden_private_file(path: Path) -> None:
    """Restrict credential files to owner read/write only (best-effort)."""
    try:
        os.chmod(path, 0o600)
    except OSError:
        pass


def sanitize_secret(value: str) -> str:
    """Strip whitespace and leading UTF-8 BOM injected by Windows pipes.

    PowerShell 5.1 `$OutputEncoding = [System.Text.Encoding]::UTF8` prepends
    U+FEFF to native stdin. str.strip() does not remove U+FEFF, so secrets
    hashed or stored with a leading BOM are rejected by upstream services.
    """
    cleaned = value.strip()
    while cleaned.startswith("\ufeff"):
        cleaned = cleaned.lstrip("\ufeff").strip()
    return cleaned


def cleanup_auth_files() -> None:
    """Remove partial auth artifacts after a failed setup."""
    for path in (CONFIG_PATH, SESSION_PATH, LEGACY_PASSWORD_TMP):
        try:
            path.unlink(missing_ok=True)
        except OSError:
            pass


def save_session(client: Client, username: str) -> None:
    """Persist username config and instagrapi settings with 0o600 permissions."""
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    # Never keep plaintext passwords on disk.
    try:
        LEGACY_PASSWORD_TMP.unlink(missing_ok=True)
    except OSError:
        pass

    CONFIG_PATH.write_text(
        json.dumps({"username": username}, indent=2, ensure_ascii=False),
        encoding="utf-8",
    )
    harden_private_file(CONFIG_PATH)

    client.dump_settings(SESSION_PATH)
    harden_private_file(SESSION_PATH)


def run_setup(username: str, password: str) -> int:
    """Authenticate with Instagram and save a private session file."""
    client = Client()
    try:
        run_sync(client.login, username, password)
        account = run_sync(client.account_info)
        save_session(client, username)
        print(
            json.dumps(
                {
                    "status": "ok",
                    "username": username,
                    "full_name": getattr(account, "full_name", "") or "",
                    "message": "Authentication successful. Session saved.",
                },
                ensure_ascii=False,
            )
        )
        return 0
    except TimeoutError as e:
        cleanup_auth_files()
        print(json.dumps({"status": "error", "message": str(e)}), file=sys.stderr)
        return 1
    except TwoFactorRequired:
        cleanup_auth_files()
        print(
            json.dumps(
                {
                    "status": "error",
                    "error_type": "two_factor_required",
                    "message": (
                        "Two-factor authentication is enabled. "
                        "Disable 2FA temporarily or use an app password / session flow, "
                        "then re-run setup."
                    ),
                }
            ),
            file=sys.stderr,
        )
        return 1
    except BadPassword:
        cleanup_auth_files()
        print(
            json.dumps({"status": "error", "message": "Invalid Instagram password."}),
            file=sys.stderr,
        )
        return 1
    except ClientError as e:
        cleanup_auth_files()
        print(
            json.dumps({"status": "error", "message": f"Instagram client error: {e}"}),
            file=sys.stderr,
        )
        return 1
    except Exception as e:
        cleanup_auth_files()
        print(
            json.dumps({"status": "error", "message": f"Authentication failed: {e}"}),
            file=sys.stderr,
        )
        return 1


def main() -> int:
    parser = argparse.ArgumentParser(description="Instagram account setup script")
    parser.add_argument("--username", required=True, help="Instagram username")
    parser.add_argument(
        "--password-stdin",
        action="store_true",
        help="Read password from stdin (preferred; use with hidden shell prompt)",
    )
    parser.add_argument(
        "--password",
        help="Instagram password (avoid when possible; prefer --password-stdin)",
    )
    args = parser.parse_args()

    password = args.password
    if args.password_stdin:
        password = sys.stdin.readline()
    if password:
        password = sanitize_secret(password)

    if not password:
        print(
            json.dumps(
                {
                    "status": "error",
                    "message": "Password is required. Use --password-stdin with a hidden prompt.",
                }
            ),
            file=sys.stderr,
        )
        return 3

    return run_setup(args.username, password)


if __name__ == "__main__":
    sys.exit(main())
