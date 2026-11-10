#!/usr/bin/env python3
import argparse
import asyncio
import json
import os
import sys
from pathlib import Path

_SCRIPTS_DIR = Path(__file__).resolve().parent
if str(_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS_DIR))

from twikit_patches import apply_twikit_patches  # noqa: E402

apply_twikit_patches()

try:
    from twikit import Client
except ImportError:
    print(json.dumps({"status": "error", "message": "twikit is not installed. Run: pip install twikit"}), file=sys.stderr)
    sys.exit(1)

CONFIG_PATH = Path.home() / ".libragent" / "x_config.json"
COOKIES_PATH = Path.home() / ".libragent" / "x_cookies.json"
OP_TIMEOUT_SEC = 60


async def await_op(awaitable):
    """Await a Twikit coroutine with a hard timeout."""
    try:
        return await asyncio.wait_for(awaitable, timeout=OP_TIMEOUT_SEC)
    except asyncio.TimeoutError as exc:
        raise TimeoutError(f"X operation timed out after {OP_TIMEOUT_SEC}s") from exc


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


def read_env_secret(env_name: str | None, secret_label: str) -> str | None:
    """
    Read a secret from the environment variable named by a --*-env option.

    LibrAgent stores a value with `storeAs: "<NAME>"`, so the agent passes only the name.
    Returns None when no name is given. Raises ValueError when the variable is empty or unset.
    """
    if not env_name:
        return None
    value = sanitize_secret(os.environ.get(env_name, ""))
    if not value:
        raise ValueError(
            f"Environment variable '{env_name}' is empty or unset. "
            f"Ask for the {secret_label} with requireUserInput=true and "
            f"storeAs=\"{env_name}\", then run this command again."
        )
    return value


def is_blocked_credentials_login_error(error_message: str) -> bool:
    normalized = error_message.lower()
    return (
        "code 34" in normalized
        or '"code":34' in normalized
        or "sorry, that page does not exist" in normalized
        or "guest token" in normalized
    )


def build_cookie_setup_command() -> str:
    script_path = Path(__file__).resolve()
    if 'com.fritzprix.libragent' in str(script_path) or 'bundled_skills' in str(script_path):
        script_path = "<skill-base-dir>/scripts/setup.py"
    return (
        f'python "{script_path}" --username "<your_username>" --email "<your_email@domain.com>" '
        "--auth-token-env X_AUTH_TOKEN --ct0-env X_CT0"
    )


def resolve_auth_inputs(
    args: argparse.Namespace,
) -> tuple[str | None, str | None, str | None, str | None]:
    """Return (password, totp_secret, auth_token, ct0) from the --*-env options."""
    return (
        read_env_secret(args.password_env, "X password"),
        read_env_secret(args.totp_secret_env, "X TOTP secret"),
        read_env_secret(args.auth_token_env, "X auth_token cookie"),
        read_env_secret(args.ct0_env, "X ct0 cookie"),
    )


async def run_setup(args, password, totp_secret, auth_token, ct0) -> int:
    # Ensure ~/.libragent exists
    CONFIG_PATH.parent.mkdir(parents=True, exist_ok=True)

    client = Client('en-US')

    try:
        if auth_token and ct0:
            client.set_cookies(
                {
                    "auth_token": auth_token,
                    "ct0": ct0,
                }
            )
        else:
            if not password:
                print(
                    json.dumps({
                        "status": "error",
                        "message": "Use --password-env for credentials login, or --auth-token-env and --ct0-env for cookie login.",
                    }),
                    file=sys.stderr,
                )
                return 3

            await await_op(
                client.login(
                    auth_info_1=args.username,
                    auth_info_2=args.email,
                    password=password,
                    totp_secret=totp_secret,
                )
            )

        client.save_cookies(str(COOKIES_PATH))
        harden_private_file(COOKIES_PATH)

        config_data = {
            "username": args.username,
            "email": args.email,
        }
        CONFIG_PATH.write_text(json.dumps(config_data, indent=2), encoding="utf-8")
        harden_private_file(CONFIG_PATH)

    except TimeoutError as e:
        CONFIG_PATH.unlink(missing_ok=True)
        COOKIES_PATH.unlink(missing_ok=True)
        print(json.dumps({"status": "error", "message": str(e)}), file=sys.stderr)
        return 1
    except Exception as e:
        # Clean up files on authentication failure to avoid partial state
        CONFIG_PATH.unlink(missing_ok=True)
        COOKIES_PATH.unlink(missing_ok=True)

        error_message = str(e)
        if is_blocked_credentials_login_error(error_message):
            print(
                json.dumps(
                    {
                        "status": "error",
                        "error_type": "credentials_login_blocked",
                        "message": "X blocked automated password login. Use browser cookie setup instead.",
                        "next_steps": [
                            "1. Log in to https://x.com in your browser.",
                            "2. Open Developer Tools > Application (or Storage) > Cookies > https://x.com.",
                            "3. Copy the auth_token and ct0 cookie values.",
                            "4. Ask for each value with requireUserInput=true and storeAs (X_AUTH_TOKEN, X_CT0), then run recommended_command.",
                        ],
                        "recommended_command": build_cookie_setup_command(),
                        "details": error_message,
                    }
                ),
                file=sys.stderr,
            )
        else:
            print(json.dumps({"status": "error", "message": f"Authentication failed: {error_message}"}), file=sys.stderr)
        return 1

    validation_warning: str | None = None
    try:
        await await_op(client.get_latest_timeline(count=1))
    except Exception as e:
        validation_warning = str(e)

    result: dict[str, str] = {
        "status": "ok",
        "username": args.username,
        "message": "Authentication successful. Session cookies saved.",
    }
    if validation_warning:
        result["warning"] = (
            "Session saved but timeline validation failed; cookies may still work for other actions."
        )
        result["validation_error"] = validation_warning
    print(json.dumps(result))
    return 0


def build_parser() -> argparse.ArgumentParser:
    # allow_abbrev=False: a removed option such as `--password <secret>` must not match `--password-env`.
    parser = argparse.ArgumentParser(description="X account setup script", allow_abbrev=False)
    parser.add_argument("--username", required=True, help="X username")
    parser.add_argument("--email", required=True, help="X email address")
    parser.add_argument("--password-env", help="Environment variable name holding the X password")
    parser.add_argument("--totp-secret-env", help="Environment variable name holding the optional 2FA TOTP secret")
    parser.add_argument("--auth-token-env", help="Environment variable name holding the browser auth_token cookie")
    parser.add_argument("--ct0-env", help="Environment variable name holding the browser ct0 cookie")
    return parser


def main() -> int:
    args = build_parser().parse_args()

    try:
        password, totp_secret, auth_token, ct0 = resolve_auth_inputs(args)
    except ValueError as e:
        print(json.dumps({"status": "error", "message": str(e)}), file=sys.stderr)
        return 3

    return asyncio.run(run_setup(args, password, totp_secret, auth_token, ct0))

if __name__ == "__main__":
    sys.exit(main())
