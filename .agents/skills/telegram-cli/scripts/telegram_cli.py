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

try:
    from telethon import TelegramClient
    from telethon.errors import (
        FloodWaitError,
        ChatAdminRequiredError,
        ChatForwardsForbiddenError,
        ChatIdInvalidError,
        ChatNotModifiedError,
        FilePartMissingError,
        PeerFloodError,
        RPCError,
        UserNotMutualContactError,
        UserNotParticipantError,
    )
    from telethon.tl.functions.messages import (
        GetMessagesRequest,
        SearchRequest,
    )
    from telethon.tl.functions.channels import GetChannelsRequest
    from telethon.tl.functions.contacts import ResolveUsernameRequest
    from telethon.tl.types import (
        Channel,
        Chat,
        User,
        Message,
        Document,
        Photo,
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
    """Get the session file path."""
    session_name = config.get("session_name", "telegram_session")
    return Path.home() / ".libragent" / session_name


# ─── Client Factory ──────────────────────────────────────────────────

def create_client(config: dict) -> TelegramClient:
    """Create and connect a TelegramClient from config."""
    session_path = get_session_path(config)
    client = TelegramClient(
        str(session_path),
        api_id=int(config["api_id"]),
        api_hash=config["api_hash"],
    )
    return client


# ─── Helpers ──────────────────────────────────────────────────────────

def resolve_chat(client: TelegramClient, chat_identifier: str) -> any:
    """Resolve a chat identifier (username, ID, or 'me') to a Peer."""
    if chat_identifier == "me":
        return "me"

    # Try as username
    if chat_identifier.startswith("@"):
        username = chat_identifier[1:]
    else:
        username = chat_identifier

    try:
        resolved = client.loop.run_until_complete(
            client(ResolveUsernameRequest(username))
        )
        return resolved.users[0] if resolved.users else None
    except Exception:
        pass

    # Try as numeric ID
    try:
        return int(chat_identifier)
    except ValueError:
        pass

    return None


def format_message(msg: Message) -> dict:
    """Format a Telegram message for output."""
    result = {
        "id": msg.id,
        "date": msg.date.isoformat() if msg.date else None,
        "text": msg.text if msg.text else "",
        "has_media": bool(msg.media),
    }

    # Add file info if media is a document
    if msg.media and isinstance(msg.media, Document):
        attr = next(
            (a for a in msg.media.attributes if isinstance(a, DocumentAttributeFilename)),
            None,
        )
        result["file_name"] = attr.file_name if attr else "unknown"
        result["file_size"] = msg.media.size
        result["mime_type"] = msg.media.mime_type

    # Add photo info
    if msg.media and isinstance(msg.media, Photo):
        result["photo"] = True

    return result


def format_chat(peer) -> dict:
    """Format a chat/channel/user for output."""
    if isinstance(peer, User):
        name = f"{peer.first_name} {peer.last_name}".strip()
        return {
            "id": peer.id,
            "name": name,
            "type": "private",
            "username": getattr(peer, "username", None),
        }
    elif isinstance(peer, Chat):
        return {
            "id": -peer.id,
            "name": peer.title,
            "type": "group",
            "member_count": getattr(peer, "participants_count", 0),
        }
    elif isinstance(peer, Channel):
        return {
            "id": -1000000000000 - peer.id if peer.id < 10**12 else -peer.id,
            "name": peer.title,
            "type": "channel" if not peer.megagroup else "supergroup",
            "subscriber_count": getattr(peer, "subscribers_count", 0),
            "username": getattr(peer, "username", None),
        }
    return {"id": getattr(peer, "id", None), "name": "Unknown", "type": "unknown"}


# ─── Actions ──────────────────────────────────────────────────────────

def action_send_message(args: argparse.Namespace, config: dict) -> int:
    """Send a message to a chat."""
    client = create_client(config)
    try:
        client.loop.run_until_complete(client.connect())
        if not client.loop.run_until_complete(client.is_user_authorized()):
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

        result = client.loop.run_until_complete(
            client.send_message(peer, args.message)
        )

        output = {
            "status": "ok",
            "message_id": result.id,
            "chat": args.chat,
            "sent_at": result.date.isoformat() if result.date else None,
        }

        if args.file:
            output["attachment"] = args.file
            # Send file
            file_path = Path(args.file)
            if file_path.exists():
                client.loop.run_until_complete(
                    client.send_file(peer, str(file_path), caption=args.message)
                )
                output["file_sent"] = True

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
        client.disconnect()


def action_get_messages(args: argparse.Namespace, config: dict) -> int:
    """Get recent messages from a chat."""
    client = create_client(config)
    try:
        client.loop.run_until_complete(client.connect())
        if not client.loop.run_until_complete(client.is_user_authorized()):
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

        limit = min(args.limit, 100)  # Telegram API max
        messages = client.loop.run_until_complete(
            client.get_messages(peer, limit=limit, offset_id=args.offset_offset)
        )

        msgs = [format_message(m) for m in messages if m]

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
        client.disconnect()


def action_list_chats(args: argparse.Namespace, config: dict) -> int:
    """List all chats."""
    client = create_client(config)
    try:
        client.loop.run_until_complete(client.connect())
        if not client.loop.run_until_complete(client.is_user_authorized()):
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
        client.disconnect()


def action_search_messages(args: argparse.Namespace, config: dict) -> int:
    """Search messages."""
    client = create_client(config)
    try:
        client.loop.run_until_complete(client.connect())
        if not client.loop.run_until_complete(client.is_user_authorized()):
            print(
                json.dumps({"status": "error", "message": "Not authorized. Run setup first."}),
                file=sys.stderr,
            )
            return 1

        limit = min(args.limit, 50)
        peer = resolve_chat(client, args.chat) if args.chat else None

        if peer:
            messages = client.loop.run_until_complete(
                client.search(peer, query=args.query, limit=limit)
            )
        else:
            # Global search across all chats
            messages = client.loop.run_until_complete(
                client.search(args.query, limit=limit)
            )

        msgs = [format_message(m) for m in messages if m]

        print(
            json.dumps({
                "status": "ok",
                "query": args.query,
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
        client.disconnect()


def action_download_file(args: argparse.Namespace, config: dict) -> int:
    """Download a file from a message."""
    client = create_client(config)
    try:
        client.loop.run_until_complete(client.connect())
        if not client.loop.run_until_complete(client.is_user_authorized()):
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

        # Get the message
        messages = client.loop.run_until_complete(
            client.get_messages(peer, ids=args.message_id)
        )
        if not messages:
            print(
                json.dumps({"status": "error", "message": "Message not found"}),
                file=sys.stderr,
            )
            return 2

        msg = messages[0]
        if not msg.media or not isinstance(msg.media, (Document, Photo)):
            print(
                json.dumps({"status": "error", "message": "Message has no downloadable file"}),
                file=sys.stderr,
            )
            return 2

        # Determine destination
        dest = Path(args.dest) if args.dest else Path.cwd()
        dest.mkdir(parents=True, exist_ok=True)

        # Get file name
        file_name = "download"
        if isinstance(msg.media, Document):
            attr = next(
                (a for a in msg.media.attributes if isinstance(a, DocumentAttributeFilename)),
                None,
            )
            if attr:
                file_name = attr.file_name
            else:
                ext = msg.media.mime_type.split("/")[-1] if msg.media.mime_type else "bin"
                file_name = f"file_{msg.id}.{ext}"
        else:
            file_name = f"photo_{msg.id}.jpg"

        out_path = dest / file_name
        client.loop.run_until_complete(
            client.download_media(msg, file=str(out_path))
        )

        print(
            json.dumps({
                "status": "ok",
                "file": str(out_path),
                "size": msg.media.size,
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
        client.disconnect()


def action_get_chat_info(args: argparse.Namespace, config: dict) -> int:
    """Get chat/channel information."""
    client = create_client(config)
    try:
        client.loop.run_until_complete(client.connect())
        if not client.loop.run_until_complete(client.is_user_authorized()):
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

        # Get recent messages for activity info
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
        client.disconnect()


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
    parser.add_argument("--offset_offset", type=int, default=0, help="Offset for pagination")
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
