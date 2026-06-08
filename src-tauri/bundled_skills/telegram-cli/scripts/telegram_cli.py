#!/usr/bin/env python3
"""
Telegram CLI — unified command dispatcher for all Telegram actions.

Usage:
  python telegram_cli.py --action <action> [options]

Actions:
  send_message    Send a text or media message
  get_messages    Read recent messages from a chat
  list_chats      List all chats/channels/groups
  search_messages Search messages by keyword
  download_file   Download a file from a message
  get_chat_info   Get chat/channel information

Exit codes:
  0 — Success
  1 — Config/session error or auth failure
  2 — Telegram API error (FloodWait, ChatNotFound, etc.)
  3 — Generic error

Prints JSON output to stdout. Errors go to stderr.
"""

import argparse
import json
import sys
from pathlib import Path
from typing import Any

try:
    from telethon import TelegramClient, utils
    from telethon.errors import (
        FloodWaitError,
        RPCError,
    )
    from telethon.tl.types import (
        Channel,
        Chat,
        User,
        Message,
        DocumentAttributeFilename,
    )
except ImportError:
    print(
        json.dumps({"status": "error", "message": "telethon is not installed. Run: pip3 install telethon"}),
        file=sys.stderr,
    )
    sys.exit(1)


# ─── Config ───────────────────────────────────────────────────────────

CONFIG_PATH = Path.home() / ".libragent" / "telegram_config.json"


def load_config() -> dict:
    """Load Telegram config from ~/.libragent/telegram_config.json."""
    if not CONFIG_PATH.exists():
        print(
            json.dumps({"status": "error", "message": "No config found. Run setup first."}),
            file=sys.stderr,
        )
        sys.exit(1)
    try:
        return json.loads(CONFIG_PATH.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError) as e:
        print(
            json.dumps({"status": "error", "message": f"Config corrupt: {e}"}),
            file=sys.stderr,
        )
        sys.exit(1)


def get_session_path(config: dict) -> Path:
    """Get the Telethon session base path (without .session suffix)."""
    session_name = config.get("session_name", "telegram_session")
    return Path.home() / ".libragent" / session_name


# ─── Client Factory ──────────────────────────────────────────────────

def create_client(config: dict) -> TelegramClient:
    """Create a TelegramClient from config."""
    session_path = get_session_path(config)
    return TelegramClient(
        str(session_path),
        api_id=int(config["api_id"]),
        api_hash=config["api_hash"],
    )


def disconnect_client(client: TelegramClient) -> None:
    """Disconnect a Telethon client, supporting sync and async disconnect()."""
    disconnect = client.disconnect()
    if disconnect is not None:
        client.loop.run_until_complete(disconnect)


def ensure_authorized(client: TelegramClient) -> bool:
    """Return True when the client session is authorized."""
    client.loop.run_until_complete(client.connect())
    return client.loop.run_until_complete(client.is_user_authorized())


def require_chat_arg(args: argparse.Namespace, action: str) -> bool:
    """Validate that --chat was provided for chat-scoped actions."""
    if args.chat:
        return True

    print(
        json.dumps({"status": "error", "message": f"Missing required argument: --chat is required for {action}."}),
        file=sys.stderr,
    )
    return False


def require_query_arg(args: argparse.Namespace) -> bool:
    """Validate that --query was provided for search_messages."""
    if args.query:
        return True

    print(
        json.dumps({"status": "error", "message": "Missing required argument: --query is required for search_messages."}),
        file=sys.stderr,
    )
    return False


# ─── Helpers ──────────────────────────────────────────────────────────

def resolve_chat(client: TelegramClient, chat_identifier: str) -> Any | None:
    """Resolve a chat identifier (username, ID, or 'me') to a Telethon entity."""
    if chat_identifier == "me":
        return "me"

    try:
        return client.loop.run_until_complete(client.get_entity(chat_identifier))
    except (ValueError, TypeError, RPCError):
        return None
    except Exception:
        return None


async def collect_messages(
    client: TelegramClient,
    entity: Any | None,
    *,
    limit: int,
    search: str | None = None,
    offset_id: int = 0,
) -> list[Message]:
    """Collect messages via iter_messages into a list."""
    messages: list[Message] = []
    kwargs: dict[str, Any] = {"limit": limit}
    if search:
        kwargs["search"] = search
    if offset_id:
        kwargs["offset_id"] = offset_id

    async for message in client.iter_messages(entity, **kwargs):
        if message:
            messages.append(message)

    return messages


def format_sender(msg: Message) -> str | None:
    """Format a message sender label when entity data is available."""
    sender = getattr(msg, "sender", None)
    if isinstance(sender, User):
        return f"{sender.first_name or ''} {sender.last_name or ''}".strip() or sender.username
    if isinstance(sender, (Chat, Channel)):
        return sender.title
    if msg.sender_id:
        return str(msg.sender_id)
    return None


def format_message(msg: Message) -> dict:
    """Format a Telegram message for output."""
    result: dict[str, Any] = {
        "id": msg.id,
        "date": msg.date.isoformat() if msg.date else None,
        "text": msg.text if msg.text else "",
        "has_media": bool(msg.media),
        "from": format_sender(msg),
    }

    if msg.document:
        attr = next(
            (a for a in msg.document.attributes if isinstance(a, DocumentAttributeFilename)),
            None,
        )
        result["file_name"] = attr.file_name if attr else "unknown"
        result["file_size"] = msg.document.size
        result["mime_type"] = msg.document.mime_type

    if msg.photo:
        result["photo"] = True

    return result


def format_chat(peer: Any) -> dict:
    """Format a chat/channel/user for output."""
    if isinstance(peer, User):
        name = f"{peer.first_name or ''} {peer.last_name or ''}".strip()
        return {
            "id": utils.get_peer_id(peer),
            "name": name,
            "type": "private",
            "username": getattr(peer, "username", None),
        }
    if isinstance(peer, Chat):
        return {
            "id": utils.get_peer_id(peer),
            "name": peer.title,
            "type": "group",
            "member_count": getattr(peer, "participants_count", 0),
        }
    if isinstance(peer, Channel):
        return {
            "id": utils.get_peer_id(peer),
            "name": peer.title,
            "type": "channel" if not peer.megagroup else "supergroup",
            "subscriber_count": getattr(peer, "subscribers_count", 0),
            "username": getattr(peer, "username", None),
        }
    return {"id": getattr(peer, "id", None), "name": "Unknown", "type": "unknown"}


# ─── Actions ──────────────────────────────────────────────────────────

def action_send_message(args: argparse.Namespace, config: dict) -> int:
    """Send a message to a chat."""
    if not require_chat_arg(args, "send_message") or not args.message:
        if not args.message:
            print(
                json.dumps({"status": "error", "message": "Missing required argument: --message is required for send_message."}),
                file=sys.stderr,
            )
        return 3

    client = create_client(config)
    try:
        if not ensure_authorized(client):
            print(
                json.dumps({"status": "error", "message": "Not authorized. Run setup first."}),
                file=sys.stderr,
            )
            return 1

        peer = resolve_chat(client, args.chat)
        if not peer:
            print(
                json.dumps({"status": "error", "message": f"Chat not found: {args.chat}"}),
                file=sys.stderr,
            )
            return 2

        if args.file:
            file_path = Path(args.file)
            if not file_path.exists():
                print(
                    json.dumps({"status": "error", "message": f"File not found: {args.file}"}),
                    file=sys.stderr,
                )
                return 3
            result = client.loop.run_until_complete(
                client.send_file(peer, str(file_path), caption=args.message)
            )
            output = {
                "status": "ok",
                "message_id": result.id,
                "chat": args.chat,
                "sent_at": result.date.isoformat() if result.date else None,
                "attachment": args.file,
                "file_sent": True,
            }
        else:
            result = client.loop.run_until_complete(
                client.send_message(peer, args.message)
            )
            output = {
                "status": "ok",
                "message_id": result.id,
                "chat": args.chat,
                "sent_at": result.date.isoformat() if result.date else None,
            }

        print(json.dumps(output))
        return 0

    except FloodWaitError as e:
        print(
            json.dumps({"status": "error", "message": f"FloodWait: wait {e.seconds} seconds"}),
            file=sys.stderr,
        )
        return 2
    except RPCError as e:
        print(
            json.dumps({"status": "error", "message": f"Telegram error: {e}"}),
            file=sys.stderr,
        )
        return 2
    except Exception as e:
        print(
            json.dumps({"status": "error", "message": f"Error: {e}"}),
            file=sys.stderr,
        )
        return 1
    finally:
        disconnect_client(client)


def action_get_messages(args: argparse.Namespace, config: dict) -> int:
    """Get recent messages from a chat."""
    if not require_chat_arg(args, "get_messages"):
        return 3

    client = create_client(config)
    try:
        if not ensure_authorized(client):
            print(
                json.dumps({"status": "error", "message": "Not authorized. Run setup first."}),
                file=sys.stderr,
            )
            return 1

        peer = resolve_chat(client, args.chat)
        if not peer:
            print(
                json.dumps({"status": "error", "message": f"Chat not found: {args.chat}"}),
                file=sys.stderr,
            )
            return 2

        limit = min(args.limit, 100)
        offset_id = args.offset_id if args.offset_offset is None else args.offset_offset
        messages = client.loop.run_until_complete(
            collect_messages(
                client,
                peer,
                limit=limit,
                offset_id=offset_id,
            )
        )
        msgs = [format_message(m) for m in messages]

        print(
            json.dumps({
                "status": "ok",
                "chat": args.chat,
                "count": len(msgs),
                "messages": msgs,
            })
        )
        return 0

    except FloodWaitError as e:
        print(
            json.dumps({"status": "error", "message": f"FloodWait: wait {e.seconds} seconds"}),
            file=sys.stderr,
        )
        return 2
    except RPCError as e:
        print(
            json.dumps({"status": "error", "message": f"Telegram error: {e}"}),
            file=sys.stderr,
        )
        return 2
    except Exception as e:
        print(
            json.dumps({"status": "error", "message": f"Error: {e}"}),
            file=sys.stderr,
        )
        return 1
    finally:
        disconnect_client(client)


def action_list_chats(args: argparse.Namespace, config: dict) -> int:
    """List all chats."""
    client = create_client(config)
    try:
        if not ensure_authorized(client):
            print(
                json.dumps({"status": "error", "message": "Not authorized. Run setup first."}),
                file=sys.stderr,
            )
            return 1

        dialogs = client.loop.run_until_complete(client.get_dialogs())
        chats = [format_chat(d.entity) for d in dialogs]

        print(
            json.dumps({
                "status": "ok",
                "count": len(chats),
                "chats": chats,
            })
        )
        return 0

    except FloodWaitError as e:
        print(
            json.dumps({"status": "error", "message": f"FloodWait: wait {e.seconds} seconds"}),
            file=sys.stderr,
        )
        return 2
    except RPCError as e:
        print(
            json.dumps({"status": "error", "message": f"Telegram error: {e}"}),
            file=sys.stderr,
        )
        return 2
    except Exception as e:
        print(
            json.dumps({"status": "error", "message": f"Error: {e}"}),
            file=sys.stderr,
        )
        return 1
    finally:
        disconnect_client(client)


def action_search_messages(args: argparse.Namespace, config: dict) -> int:
    """Search messages."""
    if not require_query_arg(args):
        return 3

    client = create_client(config)
    try:
        if not ensure_authorized(client):
            print(
                json.dumps({"status": "error", "message": "Not authorized. Run setup first."}),
                file=sys.stderr,
            )
            return 1

        limit = min(args.limit, 50)
        peer = resolve_chat(client, args.chat) if args.chat else None
        if args.chat and not peer:
            print(
                json.dumps({"status": "error", "message": f"Chat not found: {args.chat}"}),
                file=sys.stderr,
            )
            return 2

        messages = client.loop.run_until_complete(
            collect_messages(
                client,
                peer,
                limit=limit,
                search=args.query,
            )
        )
        msgs = [format_message(m) for m in messages]

        print(
            json.dumps({
                "status": "ok",
                "query": args.query,
                "chat": args.chat,
                "count": len(msgs),
                "messages": msgs,
            })
        )
        return 0

    except FloodWaitError as e:
        print(
            json.dumps({"status": "error", "message": f"FloodWait: wait {e.seconds} seconds"}),
            file=sys.stderr,
        )
        return 2
    except RPCError as e:
        print(
            json.dumps({"status": "error", "message": f"Telegram error: {e}"}),
            file=sys.stderr,
        )
        return 2
    except Exception as e:
        print(
            json.dumps({"status": "error", "message": f"Error: {e}"}),
            file=sys.stderr,
        )
        return 1
    finally:
        disconnect_client(client)


def action_download_file(args: argparse.Namespace, config: dict) -> int:
    """Download a file from a message."""
    if not require_chat_arg(args, "download_file"):
        return 3
    if not args.message_id:
        print(
            json.dumps({"status": "error", "message": "Missing required argument: --message_id is required for download_file."}),
            file=sys.stderr,
        )
        return 3

    client = create_client(config)
    try:
        if not ensure_authorized(client):
            print(
                json.dumps({"status": "error", "message": "Not authorized. Run setup first."}),
                file=sys.stderr,
            )
            return 1

        peer = resolve_chat(client, args.chat)
        if not peer:
            print(
                json.dumps({"status": "error", "message": f"Chat not found: {args.chat}"}),
                file=sys.stderr,
            )
            return 2

        res = client.loop.run_until_complete(
            client.get_messages(peer, ids=args.message_id)
        )
        if not res:
            print(
                json.dumps({"status": "error", "message": "Message not found"}),
                file=sys.stderr,
            )
            return 2

        msg = res[0] if isinstance(res, list) else res
        if not msg:
            print(
                json.dumps({"status": "error", "message": "Message not found"}),
                file=sys.stderr,
            )
            return 2

        if not msg.document and not msg.photo:
            print(
                json.dumps({"status": "error", "message": "Message has no downloadable file"}),
                file=sys.stderr,
            )
            return 2

        dest = Path(args.dest) if args.dest else Path.cwd()
        dest.mkdir(parents=True, exist_ok=True)

        file_name = "download"
        if msg.document:
            attr = next(
                (a for a in msg.document.attributes if isinstance(a, DocumentAttributeFilename)),
                None,
            )
            if attr:
                file_name = attr.file_name
            else:
                ext = msg.document.mime_type.split("/")[-1] if msg.document.mime_type else "bin"
                file_name = f"file_{msg.id}.{ext}"
        else:
            file_name = f"photo_{msg.id}.jpg"

        out_path = dest / file_name
        client.loop.run_until_complete(
            client.download_media(msg, file=str(out_path))
        )

        size = msg.document.size if msg.document else out_path.stat().st_size

        print(
            json.dumps({
                "status": "ok",
                "file": str(out_path),
                "size": size,
                "message_id": msg.id,
            })
        )
        return 0

    except FloodWaitError as e:
        print(
            json.dumps({"status": "error", "message": f"FloodWait: wait {e.seconds} seconds"}),
            file=sys.stderr,
        )
        return 2
    except RPCError as e:
        print(
            json.dumps({"status": "error", "message": f"Telegram error: {e}"}),
            file=sys.stderr,
        )
        return 2
    except Exception as e:
        print(
            json.dumps({"status": "error", "message": f"Error: {e}"}),
            file=sys.stderr,
        )
        return 1
    finally:
        disconnect_client(client)


def action_get_chat_info(args: argparse.Namespace, config: dict) -> int:
    """Get chat/channel information."""
    if not require_chat_arg(args, "get_chat_info"):
        return 3

    client = create_client(config)
    try:
        if not ensure_authorized(client):
            print(
                json.dumps({"status": "error", "message": "Not authorized. Run setup first."}),
                file=sys.stderr,
            )
            return 1

        peer = resolve_chat(client, args.chat)
        if not peer:
            print(
                json.dumps({"status": "error", "message": f"Chat not found: {args.chat}"}),
                file=sys.stderr,
            )
            return 2

        chat_info = format_chat(peer)

        recent = client.loop.run_until_complete(
            client.get_messages(peer, limit=1)
        )
        if recent:
            chat_info["last_message_date"] = recent[0].date.isoformat() if recent[0].date else None

        print(
            json.dumps({
                "status": "ok",
                "chat": chat_info,
            })
        )
        return 0

    except FloodWaitError as e:
        print(
            json.dumps({"status": "error", "message": f"FloodWait: wait {e.seconds} seconds"}),
            file=sys.stderr,
        )
        return 2
    except RPCError as e:
        print(
            json.dumps({"status": "error", "message": f"Telegram error: {e}"}),
            file=sys.stderr,
        )
        return 2
    except Exception as e:
        print(
            json.dumps({"status": "error", "message": f"Error: {e}"}),
            file=sys.stderr,
        )
        return 1
    finally:
        disconnect_client(client)


# ─── Main ─────────────────────────────────────────────────────────────

def main() -> int:
    parser = argparse.ArgumentParser(description="Telegram CLI wrapper")
    parser.add_argument(
        "--action",
        required=True,
        choices=[
            "send_message",
            "get_messages",
            "list_chats",
            "search_messages",
            "download_file",
            "get_chat_info",
        ],
        help="Action to perform",
    )
    parser.add_argument("--chat", help="Chat ID or username")
    parser.add_argument("--message", help="Message text (for send_message)")
    parser.add_argument("--query", help="Search query (for search_messages)")
    parser.add_argument("--limit", type=int, default=20, help="Number of messages (default: 20)")
    parser.add_argument(
        "--offset_id",
        type=int,
        default=0,
        help="Message ID to paginate from (older messages come before this ID)",
    )
    parser.add_argument(
        "--offset_offset",
        type=int,
        default=None,
        help="Deprecated alias for --offset_id",
    )
    parser.add_argument("--message_id", type=int, help="Message ID (for download_file)")
    parser.add_argument("--file", help="File path (for send_message)")
    parser.add_argument("--dest", help="Destination path (for download_file)")

    args = parser.parse_args()
    config = load_config()

    actions = {
        "send_message": action_send_message,
        "get_messages": action_get_messages,
        "list_chats": action_list_chats,
        "search_messages": action_search_messages,
        "download_file": action_download_file,
        "get_chat_info": action_get_chat_info,
    }

    return actions[args.action](args, config)


if __name__ == "__main__":
    sys.exit(main())
