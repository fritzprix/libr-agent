#!/usr/bin/env python3
"""
LibrAgent email client — core IMAP/SMTP operations.

Provides five actions:
  read_inbox    — list emails in inbox
  read_email    — read full email by UID
  send_email    — compose and send (with optional reply threading)
  search_email  — search by IMAP query string
  manage_email  — mark read/unread, move, delete

All output is JSON printed to stdout for the agent to parse.
Errors are printed to stderr with exit code 1.

Usage examples:
  python email_client.py --action read_inbox --limit 10 --filter unread
  python email_client.py --action read_email --uid 1042
  python email_client.py --action send_email --to "a@b.com" --subject "Hi" --body "Hello"
  python email_client.py --action search_email --query "FROM boss@corp.com"
  python email_client.py --action manage_email --uid 1042 --op mark_read
"""

import argparse
import email as email_lib
import email.header
import email.mime.multipart
import email.mime.text
import imaplib
import json
import re
import smtplib
import ssl
import sys
from datetime import datetime
from pathlib import Path
from typing import Any

CONFIG_PATH = Path.home() / ".libragent" / "email_config"


# ---------------------------------------------------------------------------
# Config loading
# ---------------------------------------------------------------------------

def load_config() -> dict:
    if not CONFIG_PATH.exists():
        error_exit("Config not found. Run setup_account.py first.")
    try:
        return json.loads(CONFIG_PATH.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        error_exit("Config file is corrupted. Run setup_account.py --reset.")


# ---------------------------------------------------------------------------
# Utilities
# ---------------------------------------------------------------------------

def error_exit(msg: str) -> None:
    print(json.dumps({"error": msg}), file=sys.stderr)
    sys.exit(1)


def decode_header_value(raw: str | None) -> str:
    """Decode RFC 2047 encoded email header value."""
    if not raw:
        return ""
    parts = email.header.decode_header(raw)
    decoded = []
    for part, charset in parts:
        if isinstance(part, bytes):
            decoded.append(part.decode(charset or "utf-8", errors="replace"))
        else:
            decoded.append(part)
    return "".join(decoded)


def parse_date(date_str: str | None) -> str:
    """Parse and normalize email date to ISO format."""
    if not date_str:
        return ""
    try:
        # Strip timezone name in parentheses
        cleaned = re.sub(r"\s*\(.*?\)\s*$", "", date_str.strip())
        parsed = email.utils.parsedate_to_datetime(cleaned)
        return parsed.isoformat()
    except Exception:
        return date_str or ""


def get_body(msg: email_lib.message.Message) -> tuple[str, list[str]]:
    """Extract plain text body and list of attachment filenames."""
    body = ""
    attachments = []

    if msg.is_multipart():
        for part in msg.walk():
            content_type = part.get_content_type()
            content_disposition = str(part.get("Content-Disposition", ""))
            filename = part.get_filename()

            if filename:
                attachments.append(decode_header_value(filename))
                continue

            if content_type == "text/plain" and "attachment" not in content_disposition:
                payload = part.get_payload(decode=True)
                charset = part.get_content_charset() or "utf-8"
                body = payload.decode(charset, errors="replace") if payload else ""
    else:
        payload = msg.get_payload(decode=True)
        charset = msg.get_content_charset() or "utf-8"
        body = payload.decode(charset, errors="replace") if payload else ""

    return body.strip(), attachments


def connect_imap(cfg: dict) -> imaplib.IMAP4_SSL:
    """Open and authenticate IMAP connection."""
    ctx = ssl.create_default_context()
    try:
        imap = imaplib.IMAP4_SSL(cfg["imap_host"], cfg["imap_port"], ssl_context=ctx)
        imap.login(cfg["email"], cfg["password"])
        return imap
    except imaplib.IMAP4.error as e:
        error_exit(f"IMAP auth failed: {e}. Run setup_account.py --reset.")
    except OSError as e:
        error_exit(f"IMAP connection failed: {e}")


def connect_smtp(cfg: dict) -> smtplib.SMTP:
    """Open and authenticate SMTP connection."""
    try:
        if not cfg.get("smtp_use_tls", True):
            # Port 465 — direct SSL
            smtp = smtplib.SMTP_SSL(cfg["smtp_host"], cfg["smtp_port"],
                                     context=ssl.create_default_context())
        else:
            # Port 587 — STARTTLS
            smtp = smtplib.SMTP(cfg["smtp_host"], cfg["smtp_port"], timeout=15)
            smtp.ehlo()
            smtp.starttls(context=ssl.create_default_context())

        smtp.login(cfg["email"], cfg["password"])
        return smtp
    except smtplib.SMTPAuthenticationError as e:
        error_exit(f"SMTP auth failed: {e}. Run setup_account.py --reset.")
    except OSError as e:
        error_exit(f"SMTP connection failed: {e}")


def fetch_message_headers(imap: imaplib.IMAP4_SSL, uid: str) -> dict[str, Any]:
    """Fetch basic header fields for a single UID."""
    _, data = imap.uid("fetch", uid, "(FLAGS BODY.PEEK[HEADER.FIELDS (FROM TO SUBJECT DATE)])")
    if not data or not data[0]:
        return {}
    raw_headers = data[0][1] if isinstance(data[0], tuple) else b""
    msg = email_lib.message_from_bytes(raw_headers)
    flags_data = data[0][0] if isinstance(data[0], tuple) else b""
    flags_str = flags_data.decode("utf-8", errors="replace") if isinstance(flags_data, bytes) else ""
    is_read = "\\Seen" in flags_str

    return {
        "uid": uid.decode() if isinstance(uid, bytes) else uid,
        "from": decode_header_value(msg.get("From")),
        "to": decode_header_value(msg.get("To")),
        "subject": decode_header_value(msg.get("Subject")),
        "date": parse_date(msg.get("Date")),
        "read": is_read,
    }


# ---------------------------------------------------------------------------
# Actions
# ---------------------------------------------------------------------------

def action_read_inbox(cfg: dict, limit: int, filter_mode: str) -> None:
    """List emails from inbox."""
    imap = connect_imap(cfg)
    imap.select("INBOX")

    criteria = {
        "unread": "UNSEEN",
        "today": f'SINCE {datetime.now().strftime("%d-%b-%Y")}',
        "all": "ALL",
    }.get(filter_mode, "UNSEEN")

    _, uid_data = imap.uid("search", None, criteria)
    uids = uid_data[0].split() if uid_data and uid_data[0] else []

    # Most recent first
    uids = uids[-limit:][::-1]

    # Total and unread counts
    _, total_data = imap.uid("search", None, "ALL")
    total = len(total_data[0].split()) if total_data and total_data[0] else 0
    _, unseen_data = imap.uid("search", None, "UNSEEN")
    unread = len(unseen_data[0].split()) if unseen_data and unseen_data[0] else 0

    emails = [fetch_message_headers(imap, uid) for uid in uids]
    imap.logout()

    print(json.dumps({
        "action": "read_inbox",
        "total": total,
        "unread": unread,
        "filter": filter_mode,
        "count": len(emails),
        "emails": emails,
    }, ensure_ascii=False, indent=2))


def action_read_email(cfg: dict, uid: str) -> None:
    """Fetch full email content by UID."""
    imap = connect_imap(cfg)
    imap.select("INBOX")

    _, data = imap.uid("fetch", uid, "(FLAGS BODY[])")
    if not data or not data[0]:
        imap.logout()
        error_exit(f"Email UID {uid} not found.")

    raw = data[0][1] if isinstance(data[0], tuple) else b""
    msg = email_lib.message_from_bytes(raw)
    body, attachments = get_body(msg)

    # Mark as read
    imap.uid("store", uid, "+FLAGS", "\\Seen")
    imap.logout()

    result = {
        "action": "read_email",
        "uid": uid,
        "from": decode_header_value(msg.get("From")),
        "to": decode_header_value(msg.get("To")),
        "cc": decode_header_value(msg.get("Cc")),
        "subject": decode_header_value(msg.get("Subject")),
        "date": parse_date(msg.get("Date")),
        "message_id": msg.get("Message-ID", ""),
        "references": msg.get("References", ""),
        "body": body,
        "attachments": attachments,
    }
    print(json.dumps(result, ensure_ascii=False, indent=2))


def action_send_email(
    cfg: dict,
    to: str,
    subject: str,
    body: str,
    cc: str | None = None,
    reply_to_uid: str | None = None,
) -> None:
    """Send an email, optionally as a reply."""
    from_addr = cfg["email"]
    msg = email.mime.multipart.MIMEMultipart()
    msg["From"] = from_addr
    msg["To"] = to
    msg["Subject"] = subject
    if cc:
        msg["Cc"] = cc

    # Threading headers for replies
    if reply_to_uid:
        imap = connect_imap(cfg)
        imap.select("INBOX")
        _, data = imap.uid("fetch", reply_to_uid, "(BODY.PEEK[HEADER.FIELDS (MESSAGE-ID REFERENCES SUBJECT)])")
        imap.logout()
        if data and data[0]:
            orig = email_lib.message_from_bytes(data[0][1] if isinstance(data[0], tuple) else b"")
            orig_mid = orig.get("Message-ID", "")
            orig_refs = orig.get("References", "")
            if orig_mid:
                msg["In-Reply-To"] = orig_mid
                msg["References"] = f"{orig_refs} {orig_mid}".strip()
            if not subject.startswith("Re:"):
                orig_subject = decode_header_value(orig.get("Subject", ""))
                msg["Subject"] = f"Re: {orig_subject}"

    msg.attach(email.mime.text.MIMEText(body, "plain", "utf-8"))

    recipients = [to] + ([cc] if cc else [])

    smtp = connect_smtp(cfg)
    smtp.sendmail(from_addr, recipients, msg.as_string())
    smtp.quit()

    sent_time = datetime.now().isoformat()
    print(json.dumps({
        "action": "send_email",
        "status": "sent",
        "from": from_addr,
        "to": to,
        "cc": cc or "",
        "subject": msg["Subject"],
        "sent_at": sent_time,
    }, ensure_ascii=False, indent=2))


def action_search_email(cfg: dict, query: str, limit: int = 20) -> None:
    """Search emails by IMAP search criteria."""
    imap = connect_imap(cfg)
    imap.select("INBOX")

    # Try UTF-8 search (RFC 6855), fall back to ASCII
    try:
        imap.literal = query.encode("utf-8")
        _, uid_data = imap.uid("search", "CHARSET", "UTF-8", query)
    except Exception:
        _, uid_data = imap.uid("search", None, query)

    uids = uid_data[0].split() if uid_data and uid_data[0] else []
    uids = uids[-limit:][::-1]

    emails = [fetch_message_headers(imap, uid) for uid in uids]
    imap.logout()

    print(json.dumps({
        "action": "search_email",
        "query": query,
        "count": len(emails),
        "emails": emails,
    }, ensure_ascii=False, indent=2))


def action_manage_email(cfg: dict, uid: str, op: str, folder: str | None = None) -> None:
    """Mark read/unread, move, or delete an email."""
    imap = connect_imap(cfg)
    imap.select("INBOX")

    if op == "mark_read":
        imap.uid("store", uid, "+FLAGS", "\\Seen")
        result_msg = "Marked as read"
    elif op == "mark_unread":
        imap.uid("store", uid, "-FLAGS", "\\Seen")
        result_msg = "Marked as unread"
    elif op == "delete":
        imap.uid("store", uid, "+FLAGS", "\\Deleted")
        imap.expunge()
        result_msg = "Deleted"
    elif op == "move":
        if not folder:
            imap.logout()
            error_exit("--folder is required for move operation.")
        # Copy then delete original
        _, copy_result = imap.uid("copy", uid, folder)
        if copy_result and copy_result[0] == "OK":
            imap.uid("store", uid, "+FLAGS", "\\Deleted")
            imap.expunge()
            result_msg = f"Moved to {folder}"
        else:
            imap.logout()
            error_exit(f"Failed to copy to folder '{folder}'. Check folder name.")
    else:
        imap.logout()
        error_exit(f"Unknown operation: {op}. Use: mark_read, mark_unread, delete, move")

    imap.logout()
    print(json.dumps({
        "action": "manage_email",
        "uid": uid,
        "op": op,
        "folder": folder or "INBOX",
        "result": result_msg,
    }, ensure_ascii=False, indent=2))


# ---------------------------------------------------------------------------
# CLI entry point
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(description="LibrAgent email client")
    parser.add_argument("--action", required=True,
                        choices=["read_inbox", "read_email", "send_email",
                                 "search_email", "manage_email"])

    # read_inbox
    parser.add_argument("--limit", type=int, default=10)
    parser.add_argument("--filter", dest="filter_mode", default="unread",
                        choices=["unread", "today", "all"])

    # read_email / manage_email
    parser.add_argument("--uid")

    # send_email
    parser.add_argument("--to")
    parser.add_argument("--subject")
    parser.add_argument("--body")
    parser.add_argument("--cc")
    parser.add_argument("--reply-to-uid")

    # search_email
    parser.add_argument("--query")

    # manage_email
    parser.add_argument("--op", choices=["mark_read", "mark_unread", "delete", "move"])
    parser.add_argument("--folder")

    args = parser.parse_args()
    cfg = load_config()

    if args.action == "read_inbox":
        action_read_inbox(cfg, args.limit, args.filter_mode)

    elif args.action == "read_email":
        if not args.uid:
            error_exit("--uid is required for read_email")
        action_read_email(cfg, args.uid)

    elif args.action == "send_email":
        if not args.to or not args.subject or not args.body:
            error_exit("--to, --subject, --body are required for send_email")
        action_send_email(cfg, args.to, args.subject, args.body,
                          cc=args.cc, reply_to_uid=args.reply_to_uid)

    elif args.action == "search_email":
        if not args.query:
            error_exit("--query is required for search_email")
        action_search_email(cfg, args.query, limit=args.limit)

    elif args.action == "manage_email":
        if not args.uid or not args.op:
            error_exit("--uid and --op are required for manage_email")
        action_manage_email(cfg, args.uid, args.op, folder=args.folder)


if __name__ == "__main__":
    main()
