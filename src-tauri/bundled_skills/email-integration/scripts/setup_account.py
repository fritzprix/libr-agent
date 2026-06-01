#!/usr/bin/env python3
"""
Email account setup for LibrAgent email-integration skill.

Collects credentials interactively (password via getpass — never visible),
tests the connection, and saves config to ~/.libragent/email_config.

Usage:
    python setup_account.py           # First-time setup
    python setup_account.py --reset   # Reset / update credentials
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
}

CONFIG_PATH = Path.home() / ".libragent" / "email_config"


def get_preset(email: str) -> dict | None:
    """Return server preset for the given email domain, or None if unknown."""
    domain = email.split("@")[-1].lower()
    return SERVER_PRESETS.get(domain)


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

    return {
        "imap_host": imap_host,
        "imap_port": int(imap_port),
        "smtp_host": smtp_host,
        "smtp_port": int(smtp_port),
    }


def test_imap(host: str, port: int, email: str, password: str) -> tuple[bool, str]:
    """Test IMAP login. Returns (success, error_message)."""
    try:
        context = ssl.create_default_context()
        with imaplib.IMAP4_SSL(host, port, ssl_context=context) as imap:
            imap.login(email, password)
        return True, ""
    except imaplib.IMAP4.error as e:
        return False, f"IMAP auth failed: {e}"
    except OSError as e:
        return False, f"Connection error: {e}"


def test_smtp(host: str, port: int, email: str, password: str) -> tuple[bool, str]:
    """Test SMTP login. Returns (success, error_message)."""
    try:
        with smtplib.SMTP(host, port, timeout=10) as smtp:
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
    CONFIG_PATH.write_text(json.dumps(config, indent=2), encoding="utf-8")
    # chmod 600 — owner read/write only
    CONFIG_PATH.chmod(stat.S_IRUSR | stat.S_IWUSR)


def run_setup(reset: bool = False) -> None:
    if CONFIG_PATH.exists() and not reset:
        print(f"\n⚠️  Config already exists at {CONFIG_PATH}")
        overwrite = input("   Overwrite? [y/N]: ").strip().lower()
        if overwrite != "y":
            print("   Setup cancelled.")
            sys.exit(0)

    print("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
    print("  LibrAgent Email Integration Setup")
    print("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
    print()

    # 1. Email address
    email = input("Email address: ").strip()
    if "@" not in email:
        print("❌ Invalid email address.")
        sys.exit(1)

    # 2. Show auth note if preset exists
    preset = get_preset(email)
    if preset and preset.get("auth_note"):
        print()
        print(f"  ℹ️  {preset['auth_note']}")

    # 3. Server settings
    server = prompt_server_settings(preset)

    # 4. Password (hidden)
    print()
    password = getpass.getpass("Password (input hidden): ")
    if not password:
        print("❌ Password cannot be empty.")
        sys.exit(1)

    # 5. Test IMAP
    print()
    print(f"  Testing IMAP connection to {server['imap_host']}:{server['imap_port']} ...")
    ok, err = test_imap(server["imap_host"], server["imap_port"], email, password)
    if not ok:
        print(f"  ❌ IMAP test failed: {err}")
        print()
        print("  Troubleshooting:")
        print("  - Gmail/Yahoo: Did you use an App Password?")
        print("  - Naver: Is IMAP enabled in mail settings?")
        print("  - Outlook: Is IMAP enabled in account settings?")
        retry = input("\n  Try different settings? [y/N]: ").strip().lower()
        if retry == "y":
            run_setup(reset=True)
        sys.exit(1)
    print("  ✓ IMAP OK")

    # 6. Test SMTP
    print(f"  Testing SMTP connection to {server['smtp_host']}:{server['smtp_port']} ...")
    ok, err = test_smtp(server["smtp_host"], server["smtp_port"], email, password)
    if not ok:
        print(f"  ⚠️  SMTP test failed: {err}")
        print("  (Read-only features will still work. Send may fail.)")
    else:
        print("  ✓ SMTP OK")

    # 7. Save
    config = {
        "email": email,
        "password": password,
        **server,
        "smtp_use_tls": server["smtp_port"] != 465,  # 465 uses SSL directly
    }
    save_config(config)

    print()
    print(f"  ✅ Config saved to {CONFIG_PATH}")
    print("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
    print()


def main() -> None:
    parser = argparse.ArgumentParser(description="LibrAgent email account setup")
    parser.add_argument("--reset", action="store_true", help="Reset existing configuration")
    args = parser.parse_args()
    run_setup(reset=args.reset)


if __name__ == "__main__":
    main()
