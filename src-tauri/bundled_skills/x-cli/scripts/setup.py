#!/usr/bin/env python3
import argparse
import asyncio
from getpass import getpass
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


def prompt_required_value(prompt: str, secret: bool = False) -> str:
    try:
        value = getpass(prompt) if secret else input(prompt)
    except EOFError:
        value = ""

    value = value.strip()
    if value:
        return value

    raise ValueError(f"Missing required input for prompt: {prompt}")


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
        return (
            'python "<skill-base-dir>/scripts/setup.py" '
            '--username "<your_username>" --email "<your_email@domain.com>" --cookie-login'
        )
    return f'python "{script_path}" --username "<your_username>" --email "<your_email@domain.com>" --cookie-login'


def resolve_auth_inputs(args: argparse.Namespace, password: str | None) -> tuple[str | None, str | None, str | None]:
    auth_token = args.auth_token
    ct0 = args.ct0

    if args.cookie_login:
        if not auth_token:
            auth_token = prompt_required_value("X auth_token cookie를 입력하세요: ", secret=True)
        if not ct0:
            ct0 = prompt_required_value("X ct0 cookie를 입력하세요: ", secret=True)

    return password, auth_token, ct0


async def run_setup(args, password, auth_token, ct0) -> int:
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
                print(json.dumps({"status": "error", "message": "Password is required for credentials login."}), file=sys.stderr)
                return 3

            await await_op(
                client.login(
                    auth_info_1=args.username,
                    auth_info_2=args.email,
                    password=password,
                    totp_secret=args.totp_secret,
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
                            "4. Re-run setup with the cookie wizard command shown in recommended_command.",
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


def main() -> int:
    parser = argparse.ArgumentParser(description="X account setup script")
    parser.add_argument("--username", required=True, help="X username")
    parser.add_argument("--email", required=True, help="X email address")
    parser.add_argument(
        "--cookie-login",
        action="store_true",
        help="Prompt for auth_token and ct0 in the terminal instead of using password login",
    )
    parser.add_argument("--password-stdin", action="store_true", help="Read password from stdin")
    parser.add_argument("--password", help="X password")
    parser.add_argument(
        "--totp-secret",
        help='Optional 2FA TOTP secret key. Pass "-" to enter it via a hidden prompt.',
    )
    parser.add_argument("--auth-token", help="Browser auth_token cookie value")
    parser.add_argument("--ct0", help="Browser ct0 cookie value")

    args = parser.parse_args()

    password = args.password
    if args.password_stdin and not (args.auth_token and args.ct0):
        password = sys.stdin.readline()
    if password:
        password = sanitize_secret(password)

    totp_secret = args.totp_secret
    if totp_secret == "-":
        try:
            totp_secret = prompt_required_value("X TOTP secret key를 입력하세요: ", secret=True)
        except ValueError as e:
            print(json.dumps({"status": "error", "message": str(e)}), file=sys.stderr)
            return 3
    args.totp_secret = totp_secret

    try:
        password, auth_token, ct0 = resolve_auth_inputs(args, password)
    except ValueError as e:
        print(json.dumps({"status": "error", "message": str(e)}), file=sys.stderr)
        return 3

    return asyncio.run(run_setup(args, password, auth_token, ct0))

if __name__ == "__main__":
    sys.exit(main())
