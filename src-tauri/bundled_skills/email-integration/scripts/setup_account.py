#!/usr/bin/env python3
"""
Email account setup for LibrAgent email-integration skill.

Supports both:
1. Interactive terminal setup for direct manual use
2. Argument-driven setup for agent workflows that can only collect one hidden prompt

Preferred agent flow:
  - Collect email address and any custom server overrides outside the script
  - Collect the password through a single hidden shell prompt in a first shell call
  - Run setup_account.py in a second visible shell call with --password-env so
    non-secret diagnostics remain visible to the agent

Usage:
    python setup_account.py                         # Interactive wizard
    python setup_account.py --reset                # Interactive reset
    python setup_account.py --email user@gmail.com --use-preset-servers \
      --password-env LIBRAGENT_EMAIL_PASSWORD
"""

import argparse
import getpass
import imaplib
import json
import os
import smtplib
import ssl
import stat
import sys
from pathlib import Path

NETWORK_TIMEOUT_SECONDS = 15

# ---------------------------------------------------------------------------
# Server presets — domain suffix → (imap_host, imap_port, smtp_host, smtp_port)
# ---------------------------------------------------------------------------
SERVER_PRESETS = {
    "gmail.com": {
        "imap_host": "imap.gmail.com",
        "imap_port": 993,
        "smtp_host": "smtp.gmail.com",
        "smtp_port": 587,
        "auth_note": (
            "Gmail requires an App Password when 2-Step Verification is enabled.\n"
            "Generate one at: https://myaccount.google.com/apppasswords"
        ),
    },
    "googlemail.com": {
        "imap_host": "imap.gmail.com",
        "imap_port": 993,
        "smtp_host": "smtp.gmail.com",
        "smtp_port": 587,
        "auth_note": (
            "Gmail requires an App Password when 2-Step Verification is enabled.\n"
            "Generate one at: https://myaccount.google.com/apppasswords"
        ),
    },
    "outlook.com": {
        "imap_host": "outlook.office365.com",
        "imap_port": 993,
        "smtp_host": "smtp.office365.com",
        "smtp_port": 587,
        "auth_note": (
            "Outlook may require an App Password if Modern Auth is enforced.\n"
            "Enable IMAP at: https://outlook.live.com/mail/options/mail/popimap"
        ),
    },
    "hotmail.com": {
        "imap_host": "outlook.office365.com",
        "imap_port": 993,
        "smtp_host": "smtp.office365.com",
        "smtp_port": 587,
        "auth_note": "Same as Outlook — enable IMAP in settings first.",
    },
    "live.com": {
        "imap_host": "outlook.office365.com",
        "imap_port": 993,
        "smtp_host": "smtp.office365.com",
        "smtp_port": 587,
        "auth_note": "Same as Outlook — enable IMAP in settings first.",
    },
    "onmicrosoft.com": {
        "imap_host": "outlook.office365.com",
        "imap_port": 993,
        "smtp_host": "smtp.office365.com",
        "smtp_port": 587,
        "auth_note": (
            "Microsoft 365 may require IMAP or SMTP AUTH to be enabled by the tenant admin.\n"
            "Custom corporate domains are not auto-detected; use explicit server flags for those."
        ),
    },
    "naver.com": {
        "imap_host": "imap.naver.com",
        "imap_port": 993,
        "smtp_host": "smtp.naver.com",
        "smtp_port": 587,
        "auth_note": (
            "Naver Mail: IMAP must be enabled manually.\n"
            "Go to: 메일 설정 > POP3/IMAP 설정 > IMAP 사용함"
        ),
    },
    "kakao.com": {
        "imap_host": "imap.kakao.com",
        "imap_port": 993,
        "smtp_host": "smtp.kakao.com",
        "smtp_port": 465,
        "auth_note": "Kakao Mail: use your Kakao account password.",
    },
    "daum.net": {
        "imap_host": "imap.daum.net",
        "imap_port": 993,
        "smtp_host": "smtp.daum.net",
        "smtp_port": 465,
        "auth_note": "Daum Mail: use your Kakao account password.",
    },
    "yahoo.com": {
        "imap_host": "imap.mail.yahoo.com",
        "imap_port": 993,
        "smtp_host": "smtp.mail.yahoo.com",
        "smtp_port": 587,
        "auth_note": (
            "Yahoo Mail requires an App Password.\n"
            "Generate one at: https://login.yahoo.com/account/security"
        ),
    },
    "icloud.com": {
        "imap_host": "imap.mail.me.com",
        "imap_port": 993,
        "smtp_host": "smtp.mail.me.com",
        "smtp_port": 587,
        "auth_note": (
            "iCloud Mail requires an app-specific password.\n"
            "Generate one at: https://appleid.apple.com"
        ),
    },
    "me.com": {
        "imap_host": "imap.mail.me.com",
        "imap_port": 993,
        "smtp_host": "smtp.mail.me.com",
        "smtp_port": 587,
        "auth_note": (
            "iCloud Mail requires an app-specific password.\n"
            "Generate one at: https://appleid.apple.com"
        ),
    },
    "mac.com": {
        "imap_host": "imap.mail.me.com",
        "imap_port": 993,
        "smtp_host": "smtp.mail.me.com",
        "smtp_port": 587,
        "auth_note": (
            "iCloud Mail requires an app-specific password.\n"
            "Generate one at: https://appleid.apple.com"
        ),
    },
}

CONFIG_PATH = Path.home() / ".libragent" / "email_config"
SERVER_FIELDS = ("imap_host", "imap_port", "smtp_host", "smtp_port")


def fail(message: str, exit_code: int = 1) -> None:
    """Exit with a concise error message."""
    print(f"❌ {message}", file=sys.stderr)
    sys.exit(exit_code)


def get_preset(email: str) -> dict | None:
    """Return server preset for the given email domain, or None if unknown."""
    domain = email.split("@")[-1].lower()
    preset = SERVER_PRESETS.get(domain)
    if preset:
        return preset

    if domain.endswith(".onmicrosoft.com"):
        return SERVER_PRESETS["onmicrosoft.com"]

    return None


def server_from_preset(preset: dict) -> dict:
    """Extract only server settings from a preset object."""
    return {field: preset[field] for field in SERVER_FIELDS}


def read_password_from_stdin() -> str:
    """Read the entire stdin stream and trim the trailing line ending."""
    password = sys.stdin.read().rstrip("\r\n")
    if not password:
        fail("Password stdin was empty.")
    return password


def resolve_password(args: argparse.Namespace, interactive: bool) -> str:
    """Load password from env/stdin or interactive getpass."""
    if args.password_env and args.password_stdin:
        fail("Use only one of --password-env or --password-stdin.")

    if args.password_env:
        password = os.environ.get(args.password_env)
        if password is None:
            fail(f"Environment variable '{args.password_env}' was not set.")
        if not password:
            fail(f"Environment variable '{args.password_env}' is empty.")
        return password

    if args.password_stdin:
        return read_password_from_stdin()

    if interactive:
        password = getpass.getpass("Password (input hidden): ")
        if not password:
            fail("Password cannot be empty.")
        return password

    fail("Password required. Use --password-env or --password-stdin.")
    return ""


def prompt_server_settings(preset: dict | None) -> dict:
    """Prompt for IMAP/SMTP settings, using preset as defaults."""
    print()
    if preset:
        imap_host = input(f"  IMAP host [{preset['imap_host']}]: ").strip() or preset["imap_host"]
        imap_port = input(f"  IMAP port [{preset['imap_port']}]: ").strip() or str(preset["imap_port"])
        smtp_host = input(f"  SMTP host [{preset['smtp_host']}]: ").strip() or preset["smtp_host"]
        smtp_port = input(f"  SMTP port [{preset['smtp_port']}]: ").strip() or str(preset["smtp_port"])
    else:
        print("  No preset found for this domain. Please enter server settings manually.")
        imap_host = input("  IMAP host (e.g. imap.example.com): ").strip()
        imap_port = input("  IMAP port [993]: ").strip() or "993"
        smtp_host = input("  SMTP host (e.g. smtp.example.com): ").strip()
        smtp_port = input("  SMTP port [587]: ").strip() or "587"

    try:
        return {
            "imap_host": imap_host,
            "imap_port": int(imap_port),
            "smtp_host": smtp_host,
            "smtp_port": int(smtp_port),
        }
    except ValueError:
        fail("IMAP/SMTP ports must be integers.")
        return {}


def resolve_server_settings(args: argparse.Namespace, email: str, interactive: bool) -> dict:
    """Resolve server settings from args, preset defaults, or interactive prompts."""
    preset = get_preset(email)
    provided_server_values = {
        "imap_host": args.imap_host,
        "imap_port": args.imap_port,
        "smtp_host": args.smtp_host,
        "smtp_port": args.smtp_port,
    }
    has_any_override = any(value is not None for value in provided_server_values.values())
    has_all_overrides = all(value is not None for value in provided_server_values.values())

    if args.use_preset_servers:
        if not preset:
            fail(
                "No preset exists for this domain. Provide --imap-host/--imap-port/"
                "--smtp-host/--smtp-port instead."
            )
        return server_from_preset(preset)

    if has_any_override:
        if not has_all_overrides:
            fail(
                "Provide all four server overrides together: --imap-host --imap-port "
                "--smtp-host --smtp-port."
            )
        return {
            "imap_host": args.imap_host,
            "imap_port": args.imap_port,
            "smtp_host": args.smtp_host,
            "smtp_port": args.smtp_port,
        }

    if interactive:
        return prompt_server_settings(preset)

    if preset:
        return server_from_preset(preset)

    fail(
        "No preset exists for this domain. Provide --imap-host --imap-port "
        "--smtp-host --smtp-port."
    )
    return {}


def confirm_overwrite(args: argparse.Namespace, interactive: bool) -> None:
    """Handle existing config file overwrite rules."""
    if not CONFIG_PATH.exists() or args.reset or args.force_overwrite:
        return

    if interactive:
        print(f"\n⚠️  Config already exists at {CONFIG_PATH}")
        overwrite = input("   Overwrite? [y/N]: ").strip().lower()
        if overwrite == "y":
            return
        print("   Setup cancelled.")
        sys.exit(0)

    fail("Config already exists. Re-run with --reset or --force-overwrite.")


def test_imap(host: str, port: int, email: str, password: str) -> tuple[bool, str]:
    """Test IMAP login. Returns (success, error_message)."""
    try:
        context = ssl.create_default_context()
        with imaplib.IMAP4_SSL(
            host,
            port,
            ssl_context=context,
            timeout=NETWORK_TIMEOUT_SECONDS,
        ) as imap:
            imap.login(email, password)
        return True, ""
    except imaplib.IMAP4.error as e:
        return False, f"IMAP auth failed: {e}"
    except OSError as e:
        return False, f"Connection error: {e}"


def test_smtp(host: str, port: int, email: str, password: str) -> tuple[bool, str]:
    """Test SMTP login. Returns (success, error_message)."""
    try:
        if port == 465:
            with smtplib.SMTP_SSL(
                host,
                port,
                timeout=NETWORK_TIMEOUT_SECONDS,
                context=ssl.create_default_context(),
            ) as smtp:
                smtp.login(email, password)
        else:
            with smtplib.SMTP(host, port, timeout=NETWORK_TIMEOUT_SECONDS) as smtp:
                smtp.ehlo()
                smtp.starttls(context=ssl.create_default_context())
                smtp.login(email, password)
        return True, ""
    except smtplib.SMTPAuthenticationError as e:
        return False, f"SMTP auth failed: {e}"
    except OSError as e:
        return False, f"Connection error: {e}"


def save_config(config: dict) -> None:
    """Save config JSON with restricted permissions (600)."""
    CONFIG_PATH.parent.mkdir(parents=True, exist_ok=True)
    CONFIG_PATH.write_text(json.dumps(config, indent=2, ensure_ascii=False), encoding="utf-8")
    CONFIG_PATH.chmod(stat.S_IRUSR | stat.S_IWUSR)


def run_setup(args: argparse.Namespace) -> None:
    interactive = sys.stdin.isatty() and sys.stdout.isatty()
    confirm_overwrite(args, interactive)

    print("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
    print("  LibrAgent Email Integration Setup")
    print("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
    print()

    if args.email:
        email = args.email.strip()
    elif interactive:
        email = input("Email address: ").strip()
    else:
        fail("Email address required. Provide --email.")

    if "@" not in email:
        fail("Invalid email address.")

    preset = get_preset(email)
    if preset and preset.get("auth_note"):
        print()
        print(f"  ℹ️  {preset['auth_note']}")

    server = resolve_server_settings(args, email, interactive)

    print()
    password = resolve_password(args, interactive)

    print()
    print(f"  Testing IMAP connection to {server['imap_host']}:{server['imap_port']} ...")
    ok, err = test_imap(server["imap_host"], server["imap_port"], email, password)
    if not ok:
        print(f"  ❌ IMAP test failed: {err}")
        print()
        print("  Troubleshooting:")
        print("  - Gmail/Yahoo/iCloud: Did you use an App Password?")
        print("  - Naver: Is IMAP enabled in mail settings?")
        print("  - Outlook/M365: Is IMAP or SMTP AUTH enabled in account settings?")
        if interactive:
            retry = input("\n  Try different settings? [y/N]: ").strip().lower()
            if retry == "y":
                rerun_args = argparse.Namespace(**vars(args))
                rerun_args.reset = True
                rerun_args.use_preset_servers = False
                rerun_args.imap_host = None
                rerun_args.imap_port = None
                rerun_args.smtp_host = None
                rerun_args.smtp_port = None
                run_setup(rerun_args)
                return
        sys.exit(1)
    print("  ✓ IMAP OK")

    print(f"  Testing SMTP connection to {server['smtp_host']}:{server['smtp_port']} ...")
    ok, err = test_smtp(server["smtp_host"], server["smtp_port"], email, password)
    if not ok:
        print(f"  ⚠️  SMTP test failed: {err}")
        print("  (Read-only features will still work. Send may fail.)")
    else:
        print("  ✓ SMTP OK")

    config = {
        "email": email,
        "password": password,
        **server,
        "smtp_use_tls": server["smtp_port"] != 465,
    }
    save_config(config)

    print()
    print(f"  ✅ Config saved to {CONFIG_PATH}")
    print("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
    print()


def build_parser() -> argparse.ArgumentParser:
    """Create CLI parser."""
    parser = argparse.ArgumentParser(description="LibrAgent email account setup")
    parser.add_argument("--reset", action="store_true", help="Reset existing configuration")
    parser.add_argument("--force-overwrite", action="store_true", help="Overwrite existing config without prompt")
    parser.add_argument("--email", help="Email address to configure")
    parser.add_argument(
        "--use-preset-servers",
        action="store_true",
        help="Use the known preset for the email domain without prompting for IMAP/SMTP fields",
    )
    parser.add_argument("--imap-host", help="Override IMAP host")
    parser.add_argument("--imap-port", type=int, help="Override IMAP port")
    parser.add_argument("--smtp-host", help="Override SMTP host")
    parser.add_argument("--smtp-port", type=int, help="Override SMTP port")
    parser.add_argument(
        "--password-env",
        help="Read the password from the named environment variable",
    )
    parser.add_argument(
        "--password-stdin",
        action="store_true",
        help="Read the password from stdin",
    )
    return parser


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()
    run_setup(args)


if __name__ == "__main__":
    main()
