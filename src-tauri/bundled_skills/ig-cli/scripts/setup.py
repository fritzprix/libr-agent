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
    """Strip whitespace and any leading UTF-8 BOM (str.strip() keeps U+FEFF)."""
    cleaned = value.strip()
    while cleaned.startswith("\ufeff"):
        cleaned = cleaned.lstrip("\ufeff").strip()
    return cleaned


def read_env_secret(env_name: str) -> str:
    """
    Read the password from the environment variable named by --password-env.

    LibrAgent stores a value with `storeAs: "<NAME>"`, so the agent passes only the name.
    Raises ValueError when the variable is empty or unset.
    """
    value = sanitize_secret(os.environ.get(env_name, ""))
    if not value:
        raise ValueError(
            f"Environment variable '{env_name}' is empty or unset. "
            f"Ask for the Instagram password with requireUserInput=true and "
            f"storeAs=\"{env_name}\", then run this command again."
        )
    return value


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


def build_parser() -> argparse.ArgumentParser:
    # allow_abbrev=False: a removed option such as `--password <secret>` must not match `--password-env`.
    parser = argparse.ArgumentParser(description="Instagram account setup script", allow_abbrev=False)
    parser.add_argument("--username", required=True, help="Instagram username")
    parser.add_argument(
        "--password-env",
        required=True,
        help="Environment variable name holding the Instagram password",
    )
    return parser


def main() -> int:
    args = build_parser().parse_args()

    try:
        password = read_env_secret(args.password_env)
    except ValueError as e:
        print(json.dumps({"status": "error", "message": str(e)}), file=sys.stderr)
        return 3

    return run_setup(args.username, password)


if __name__ == "__main__":
    sys.exit(main())
