#!/usr/bin/env python3
"""
LibrAgent Telegram Setup Script.
Handles Telegram client configuration and MTProto session generation.
All comments are written in English.
"""

import argparse
import json
import os
import sys
from pathlib import Path

try:
    from telethon import TelegramClient
    from telethon.errors import (
        SessionPasswordNeededError,
        PhoneCodeInvalidError,
        PhoneCodeExpiredError,
        PasswordHashInvalidError,
        FloodWaitError,
        RPCError,
    )
except ImportError:
    print(
        json.dumps({"status": "error", "message": "telethon is not installed. Run: pip3 install telethon"}),
        file=sys.stderr,
    )
    sys.exit(1)

CONFIG_DIR = Path.home() / ".libragent"
CONFIG_PATH = CONFIG_DIR / "telegram_config.json"
SESSION_NAME = "telegram_session"
SESSION_PATH = CONFIG_DIR / f"{SESSION_NAME}.session"
CLIENT_SESSION_PATH = CONFIG_DIR / SESSION_NAME

def save_config(api_id: int, api_hash: str, phone: str, phone_code_hash: str = "") -> None:
    """Save configuration to ~/.libragent/telegram_config.json."""
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    config_data = {
        "api_id": api_id,
        "api_hash": api_hash,
        "phone": phone,
        "session_name": SESSION_NAME,
    }
    if phone_code_hash:
        config_data["phone_code_hash"] = phone_code_hash
        
    CONFIG_PATH.write_text(json.dumps(config_data, indent=2), encoding="utf-8")
    try:
        os.chmod(CONFIG_PATH, 0o600)
    except OSError:
        pass

def load_config() -> dict:
    """Load configuration from ~/.libragent/telegram_config.json."""
    if not CONFIG_PATH.exists():
        return {}
    try:
        return json.loads(CONFIG_PATH.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return {}

def action_send_code(args: argparse.Namespace) -> int:
    """Send verification code to the phone number."""
    api_id = args.api_id
    api_hash = args.api_hash
    phone = args.phone

    if not api_id or not api_hash or not phone:
        config = load_config()
        api_id = api_id or config.get("api_id")
        api_hash = api_hash or config.get("api_hash")
        phone = phone or config.get("phone")

    if not api_id or not api_hash or not phone:
        print(
            json.dumps({"status": "error", "message": "Missing required arguments: --api-id, --api-hash, and --phone are required."}),
            file=sys.stderr,
        )
        return 1

    CONFIG_DIR.mkdir(parents=True, exist_ok=True)

    client = TelegramClient(str(CLIENT_SESSION_PATH), int(api_id), api_hash)
    try:
        client.loop.run_until_complete(client.connect())
        sent_code = client.loop.run_until_complete(client.send_code_request(phone))
        phone_code_hash = sent_code.phone_code_hash
        
        save_config(int(api_id), api_hash, phone, phone_code_hash)
        
        print(
            json.dumps({
                "status": "code_sent",
                "phone_code_hash": phone_code_hash,
                "message": "Verification code has been sent to your Telegram account/phone."
            })
        )
        return 0
    except FloodWaitError as e:
        print(
            json.dumps({"status": "error", "message": f"FloodWait: Please wait {e.seconds} seconds before retrying."}),
            file=sys.stderr,
        )
        return 2
    except RPCError as e:
        print(
            json.dumps({"status": "error", "message": f"Telegram API error: {e}"}),
            file=sys.stderr,
        )
        return 2
    except Exception as e:
        print(
            json.dumps({"status": "error", "message": f"Unexpected error: {e}"}),
            file=sys.stderr,
        )
        return 3
    finally:
        client.loop.run_until_complete(client.disconnect())

def action_sign_in(args: argparse.Namespace) -> int:
    """Complete authorization using code and optionally 2FA password."""
    config = load_config()
    api_id = args.api_id or config.get("api_id")
    api_hash = args.api_hash or config.get("api_hash")
    phone = args.phone or config.get("phone")
    phone_code_hash = config.get("phone_code_hash")

    if not api_id or not api_hash or not phone:
        print(
            json.dumps({"status": "error", "message": "Missing configuration. Run send_code first."}),
            file=sys.stderr,
        )
        return 1

    code = ""
    password = ""

    if args.code_env:
        code = os.environ.get(args.code_env, "")
    elif args.code_stdin:
        code = sys.stdin.read().strip()

    if args.password_env:
        password = os.environ.get(args.password_env, "")
    elif args.password_stdin:
        password = sys.stdin.read().strip()

    client = TelegramClient(str(CLIENT_SESSION_PATH), int(api_id), api_hash)
    try:
        client.loop.run_until_complete(client.connect())

        if client.loop.run_until_complete(client.is_user_authorized()):
            print(json.dumps({"status": "ok", "message": "Already authorized."}))
            return 0

        if password:
            try:
                client.loop.run_until_complete(client.sign_in(password=password))
                print(json.dumps({"status": "ok", "message": "Successfully authenticated with 2FA password."}))
                return 0
            except PasswordHashInvalidError:
                print(
                    json.dumps({"status": "error", "message": "Invalid 2FA password."}),
                    file=sys.stderr,
                )
                return 2
        
        if code:
            try:
                client.loop.run_until_complete(
                    client.sign_in(phone=phone, code=code, phone_code_hash=phone_code_hash)
                )
                print(json.dumps({"status": "ok", "message": "Successfully authenticated and session saved."}))
                return 0
            except SessionPasswordNeededError:
                print(
                    json.dumps({
                        "status": "password_needed",
                        "message": "Two-factor authentication (2FA) is enabled. Please enter your 2FA password."
                    })
                )
                return 0
            except PhoneCodeInvalidError:
                print(
                    json.dumps({"status": "error", "message": "The verification code is invalid."}),
                    file=sys.stderr,
                )
                return 2
            except PhoneCodeExpiredError:
                print(
                    json.dumps({"status": "error", "message": "The verification code has expired."}),
                    file=sys.stderr,
                )
                return 2

        print(
            json.dumps({"status": "error", "message": "No code or password provided via env vars."}),
            file=sys.stderr,
        )
        return 1

    except FloodWaitError as e:
        print(
            json.dumps({"status": "error", "message": f"FloodWait: Please wait {e.seconds} seconds."}),
            file=sys.stderr,
        )
        return 2
    except RPCError as e:
        print(
            json.dumps({"status": "error", "message": f"Telegram API error: {e}"}),
            file=sys.stderr,
        )
        return 2
    except Exception as e:
        print(
            json.dumps({"status": "error", "message": f"Unexpected error: {e}"}),
            file=sys.stderr,
        )
        return 3
    finally:
        client.loop.run_until_complete(client.disconnect())

def main() -> int:
    parser = argparse.ArgumentParser(description="Telegram setup script")
    parser.add_argument("--action", required=True, choices=["send_code", "sign_in"], help="Action to perform")
    parser.add_argument("--api-id", type=int, help="Telegram API ID")
    parser.add_argument("--api-hash", help="Telegram API Hash")
    parser.add_argument("--phone", help="Telegram phone number with country code")
    parser.add_argument("--code-env", help="Environment variable name holding the verification code")
    parser.add_argument("--password-env", help="Environment variable name holding the 2FA password")
    parser.add_argument("--code-stdin", action="store_true", help="Read verification code from stdin")
    parser.add_argument("--password-stdin", action="store_true", help="Read 2FA password from stdin")

    args = parser.parse_args()

    if args.action == "send_code":
        return action_send_code(args)
    elif args.action == "sign_in":
        return action_sign_in(args)

    return 1

if __name__ == "__main__":
    sys.exit(main())
