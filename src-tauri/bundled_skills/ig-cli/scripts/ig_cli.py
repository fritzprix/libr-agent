#!/usr/bin/env python3
"""
Instagram CLI — unified command dispatcher via instagrapi.

Exit codes:
  0 — Success
  1 — Config/session error or auth failure
  2 — Instagram API / client error
  3 — Missing or invalid arguments

Success JSON goes to stdout. Errors go to stderr as JSON.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from concurrent.futures import ThreadPoolExecutor
from concurrent.futures import TimeoutError as FuturesTimeout
from pathlib import Path
from typing import Any, Callable, TypeVar

try:
    from instagrapi import Client
    from instagrapi.exceptions import ClientError, LoginRequired, RateLimitError
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

CONFIG_PATH = Path.home() / ".libragent" / "ig_config.json"
SESSION_PATH = Path.home() / ".libragent" / "ig_session.json"
OP_TIMEOUT_SEC = 60
MAX_MEDIA_BYTES = 100 * 1024 * 1024

T = TypeVar("T")


def emit_error(payload: dict[str, Any], code: int = 1) -> None:
    print(json.dumps(payload, ensure_ascii=False), file=sys.stderr)
    raise SystemExit(code)


def emit_ok(payload: dict[str, Any]) -> None:
    print(json.dumps(payload, ensure_ascii=False, default=str))


def harden_private_file(path: Path) -> None:
    """Restrict credential files to owner read/write only (best-effort)."""
    try:
        os.chmod(path, 0o600)
    except OSError:
        pass


def run_sync(fn: Callable[..., T], *args: Any, timeout: float = OP_TIMEOUT_SEC, **kwargs: Any) -> T:
    """Run a blocking instagrapi call with a hard timeout."""
    with ThreadPoolExecutor(max_workers=1) as pool:
        future = pool.submit(fn, *args, **kwargs)
        try:
            return future.result(timeout=timeout)
        except FuturesTimeout as exc:
            raise TimeoutError(f"Instagram operation timed out after {int(timeout)}s") from exc


def validate_media_path(file_path: str) -> Path:
    """Ensure media path exists, is within size limits, and is outside ~/.libragent."""
    path = Path(file_path)
    if not path.exists() or not path.is_file():
        emit_error({"status": "error", "message": f"File not found: {file_path}"}, code=3)

    try:
        resolved = path.resolve()
        config_dir = CONFIG_PATH.parent.resolve()
        if (
            resolved == CONFIG_PATH.resolve()
            or resolved == SESSION_PATH.resolve()
            or config_dir in resolved.parents
            or resolved == config_dir
        ):
            emit_error(
                {
                    "status": "error",
                    "message": "Access denied: cannot read media from the LibrAgent config directory.",
                },
                code=3,
            )
        size = path.stat().st_size
    except SystemExit:
        raise
    except Exception as e:
        emit_error({"status": "error", "message": f"Path validation failed: {e}"}, code=3)

    if size > MAX_MEDIA_BYTES:
        emit_error(
            {
                "status": "error",
                "message": (
                    f"File size exceeds the limit of {MAX_MEDIA_BYTES} bytes "
                    f"(got {size} bytes)."
                ),
            },
            code=3,
        )
    return path


def save_session(client: Client) -> None:
    """Refresh session settings on disk after a successful authenticated call."""
    SESSION_PATH.parent.mkdir(parents=True, exist_ok=True)
    client.dump_settings(SESSION_PATH)
    harden_private_file(SESSION_PATH)
    if CONFIG_PATH.exists():
        harden_private_file(CONFIG_PATH)


def get_client() -> Client:
    """Load private session settings and verify they still authorize."""
    if not CONFIG_PATH.exists() or not SESSION_PATH.exists():
        emit_error(
            {
                "status": "error",
                "message": "Not logged in. Run setup.py first.",
                "action": "setup",
            },
            code=1,
        )

    client = Client()
    try:
        client.load_settings(SESSION_PATH)
    except Exception as e:
        emit_error(
            {
                "status": "error",
                "message": f"Failed to load session: {e}. Re-run setup.py.",
                "action": "setup",
            },
            code=1,
        )

    try:
        run_sync(client.account_info)
    except TimeoutError as e:
        emit_error({"status": "error", "message": str(e)}, code=2)
    except LoginRequired:
        emit_error(
            {
                "status": "error",
                "message": "Session expired. Re-run setup.py (password is never stored on disk).",
                "action": "setup",
            },
            code=1,
        )
    except RateLimitError as e:
        emit_error(
            {"status": "error", "message": f"Rate limited while verifying session: {e}"},
            code=2,
        )
    except ClientError as e:
        emit_error(
            {"status": "error", "message": f"Instagram client error while verifying session: {e}"},
            code=2,
        )

    harden_private_file(CONFIG_PATH)
    harden_private_file(SESSION_PATH)
    return client


def media_type_label(media_type: int | None) -> str:
    if media_type == 1:
        return "photo"
    if media_type == 2:
        return "video"
    if media_type == 8:
        return "carousel"
    return "unknown"


def build_caption(caption: str | None, tags: str | None) -> str:
    text = (caption or "").strip()
    if tags:
        hashtags = " ".join(f"#{t.strip().lstrip('#')}" for t in tags.split(",") if t.strip())
        if hashtags:
            text = f"{text}\n{hashtags}".strip()
    return text


def action_post_photo(client: Client, args: argparse.Namespace) -> None:
    media_path = validate_media_path(args.file) if args.file else None
    if media_path is None:
        emit_error({"status": "error", "message": "--file is required."}, code=3)

    caption = build_caption(args.caption, args.tags)
    try:
        media = run_sync(client.photo_upload, media_path, caption)
        save_session(client)
        result: dict[str, Any] = {
            "status": "ok",
            "action": "post_photo",
            "post_id": str(media.pk),
            "caption": caption,
        }
        if args.location:
            result["location"] = args.location
            result["warning"] = (
                "location flag is recorded in output only; "
                "Instagram location tagging is not applied in this version."
            )
        emit_ok(result)
    except TimeoutError as e:
        emit_error({"status": "error", "message": str(e)}, code=2)
    except LoginRequired:
        emit_error(
            {"status": "error", "message": "Session expired. Re-run setup.py.", "action": "setup"},
            code=1,
        )
    except RateLimitError as e:
        emit_error({"status": "error", "message": f"Rate limited: {e}"}, code=2)
    except ClientError as e:
        emit_error({"status": "error", "message": f"Instagram client error: {e}"}, code=2)
    except Exception as e:
        emit_error({"status": "error", "message": str(e)}, code=2)


def action_post_reel(client: Client, args: argparse.Namespace) -> None:
    media_path = validate_media_path(args.file) if args.file else None
    if media_path is None:
        emit_error({"status": "error", "message": "--file is required."}, code=3)

    caption = build_caption(args.caption, args.tags)
    thumbnail = validate_media_path(args.thumbnail) if args.thumbnail else None

    try:
        if thumbnail is not None:
            media = run_sync(client.clip_upload, media_path, caption, thumbnail=thumbnail)
        else:
            media = run_sync(client.clip_upload, media_path, caption)
        save_session(client)
        emit_ok(
            {
                "status": "ok",
                "action": "post_reel",
                "post_id": str(media.pk),
                "caption": caption,
            }
        )
    except TimeoutError as e:
        emit_error({"status": "error", "message": str(e)}, code=2)
    except LoginRequired:
        emit_error(
            {"status": "error", "message": "Session expired. Re-run setup.py.", "action": "setup"},
            code=1,
        )
    except RateLimitError as e:
        emit_error({"status": "error", "message": f"Rate limited: {e}"}, code=2)
    except ClientError as e:
        emit_error({"status": "error", "message": f"Instagram client error: {e}"}, code=2)
    except Exception as e:
        emit_error({"status": "error", "message": str(e)}, code=2)


def action_reply_comment(client: Client, args: argparse.Namespace) -> None:
    if not args.post_id or not args.parent_comment_id or not args.message:
        emit_error(
            {
                "status": "error",
                "message": "--post-id, --parent-comment-id, and --message are required.",
            },
            code=3,
        )
    try:
        comment = run_sync(
            client.media_comment,
            args.post_id,
            args.message,
            replied_to_comment_id=int(args.parent_comment_id),
        )
        save_session(client)
        emit_ok(
            {
                "status": "ok",
                "action": "reply_comment",
                "comment_id": str(comment.pk),
                "post_id": args.post_id,
                "message": args.message,
            }
        )
    except LoginRequired:
        emit_error(
            {"status": "error", "message": "Session expired. Re-run setup.py.", "action": "setup"},
            code=1,
        )
    except Exception as e:
        emit_error({"status": "error", "message": str(e)}, code=2)


def action_add_comment(client: Client, args: argparse.Namespace) -> None:
    if not args.post_id or not args.message:
        emit_error(
            {"status": "error", "message": "--post-id and --message are required."},
            code=3,
        )
    try:
        comment = run_sync(client.media_comment, args.post_id, args.message)
        save_session(client)
        emit_ok(
            {
                "status": "ok",
                "action": "add_comment",
                "comment_id": str(comment.pk),
                "post_id": args.post_id,
                "message": args.message,
            }
        )
    except LoginRequired:
        emit_error(
            {"status": "error", "message": "Session expired. Re-run setup.py.", "action": "setup"},
            code=1,
        )
    except Exception as e:
        emit_error({"status": "error", "message": str(e)}, code=2)


def action_like_post(client: Client, args: argparse.Namespace) -> None:
    if not args.post_id:
        emit_error({"status": "error", "message": "--post-id is required."}, code=3)
    try:
        run_sync(client.media_like, args.post_id)
        save_session(client)
        emit_ok({"status": "ok", "action": "like_post", "post_id": args.post_id})
    except LoginRequired:
        emit_error(
            {"status": "error", "message": "Session expired. Re-run setup.py.", "action": "setup"},
            code=1,
        )
    except Exception as e:
        emit_error({"status": "error", "message": str(e)}, code=2)


def action_unlike_post(client: Client, args: argparse.Namespace) -> None:
    if not args.post_id:
        emit_error({"status": "error", "message": "--post-id is required."}, code=3)
    try:
        run_sync(client.media_unlike, args.post_id)
        save_session(client)
        emit_ok({"status": "ok", "action": "unlike_post", "post_id": args.post_id})
    except LoginRequired:
        emit_error(
            {"status": "error", "message": "Session expired. Re-run setup.py.", "action": "setup"},
            code=1,
        )
    except Exception as e:
        emit_error({"status": "error", "message": str(e)}, code=2)


def action_follow_user(client: Client, args: argparse.Namespace) -> None:
    if not args.username:
        emit_error({"status": "error", "message": "--username is required."}, code=3)
    try:
        user_id = run_sync(client.user_id_from_username, args.username)
        run_sync(client.user_follow, user_id)
        save_session(client)
        emit_ok({"status": "ok", "action": "follow_user", "username": args.username})
    except LoginRequired:
        emit_error(
            {"status": "error", "message": "Session expired. Re-run setup.py.", "action": "setup"},
            code=1,
        )
    except Exception as e:
        emit_error({"status": "error", "message": str(e)}, code=2)


def action_unfollow_user(client: Client, args: argparse.Namespace) -> None:
    if not args.username:
        emit_error({"status": "error", "message": "--username is required."}, code=3)
    try:
        user_id = run_sync(client.user_id_from_username, args.username)
        run_sync(client.user_unfollow, user_id)
        save_session(client)
        emit_ok({"status": "ok", "action": "unfollow_user", "username": args.username})
    except LoginRequired:
        emit_error(
            {"status": "error", "message": "Session expired. Re-run setup.py.", "action": "setup"},
            code=1,
        )
    except Exception as e:
        emit_error({"status": "error", "message": str(e)}, code=2)


def _serialize_media(item: Any) -> dict[str, Any]:
    caption = getattr(item, "caption_text", None) or getattr(item, "caption", None) or ""
    if not isinstance(caption, str):
        caption = str(caption) if caption else ""
    return {
        "id": str(getattr(item, "pk", "")),
        "caption": caption[:100],
        "media_type": media_type_label(getattr(item, "media_type", None)),
        "like_count": getattr(item, "like_count", 0) or 0,
        "comment_count": getattr(item, "comment_count", 0) or 0,
        "taken_at": getattr(item, "taken_at", None),
    }


def action_get_feed(client: Client, args: argparse.Namespace) -> None:
    limit = args.limit or 10
    try:
        posts: list[dict[str, Any]] = []
        try:
            feed = run_sync(client.get_timeline_feed)
            for entry in feed.get("feed_items", []) or []:
                if len(posts) >= limit:
                    break
                media = entry.get("media_or_ad") or entry.get("media")
                if not isinstance(media, dict) or "pk" not in media:
                    continue
                caption_obj = media.get("caption") or {}
                caption_text = ""
                if isinstance(caption_obj, dict):
                    caption_text = str(caption_obj.get("text") or "")
                elif isinstance(caption_obj, str):
                    caption_text = caption_obj
                posts.append(
                    {
                        "id": str(media.get("pk")),
                        "caption": caption_text[:100],
                        "media_type": media_type_label(media.get("media_type")),
                        "like_count": media.get("like_count") or 0,
                        "comment_count": media.get("comment_count") or 0,
                        "taken_at": media.get("taken_at") or media.get("device_timestamp"),
                    }
                )
        except Exception:
            posts = []

        if not posts:
            user_id = client.user_id
            medias = run_sync(client.user_medias, user_id, amount=limit)
            posts = [_serialize_media(item) for item in medias[:limit]]

        save_session(client)
        emit_ok({"status": "ok", "action": "get_feed", "posts": posts, "count": len(posts)})
    except LoginRequired:
        emit_error(
            {"status": "error", "message": "Session expired. Re-run setup.py.", "action": "setup"},
            code=1,
        )
    except Exception as e:
        emit_error({"status": "error", "message": str(e)}, code=2)


def action_get_user_posts(client: Client, args: argparse.Namespace) -> None:
    if not args.username:
        emit_error({"status": "error", "message": "--username is required."}, code=3)
    limit = args.limit or 10
    try:
        user_id = run_sync(client.user_id_from_username, args.username)
        medias = run_sync(client.user_medias, user_id, amount=limit)
        posts = [_serialize_media(item) for item in medias[:limit]]
        save_session(client)
        emit_ok(
            {
                "status": "ok",
                "action": "get_user_posts",
                "username": args.username,
                "posts": posts,
                "count": len(posts),
            }
        )
    except LoginRequired:
        emit_error(
            {"status": "error", "message": "Session expired. Re-run setup.py.", "action": "setup"},
            code=1,
        )
    except Exception as e:
        emit_error({"status": "error", "message": str(e)}, code=2)


def action_get_comments(client: Client, args: argparse.Namespace) -> None:
    if not args.post_id:
        emit_error({"status": "error", "message": "--post-id is required."}, code=3)
    try:
        comments = run_sync(client.media_comments, args.post_id)
        comment_list = []
        for c in comments:
            user = getattr(c, "user", None)
            username = getattr(user, "username", "") if user is not None else ""
            comment_list.append(
                {
                    "id": str(c.pk),
                    "user": username,
                    "text": c.text,
                    "created_at": str(getattr(c, "created_at_utc", None) or getattr(c, "created_at", "")),
                }
            )
        save_session(client)
        emit_ok(
            {
                "status": "ok",
                "action": "get_comments",
                "post_id": args.post_id,
                "comments": comment_list,
                "count": len(comment_list),
            }
        )
    except LoginRequired:
        emit_error(
            {"status": "error", "message": "Session expired. Re-run setup.py.", "action": "setup"},
            code=1,
        )
    except Exception as e:
        emit_error({"status": "error", "message": str(e)}, code=2)


def action_get_profile(client: Client, _args: argparse.Namespace) -> None:
    try:
        account = run_sync(client.account_info)
        user = None
        try:
            user = run_sync(client.user_info, str(account.pk))
        except Exception:
            user = None
        save_session(client)
        emit_ok(
            {
                "status": "ok",
                "action": "get_profile",
                "username": account.username,
                "full_name": account.full_name,
                "biography": getattr(account, "biography", "") or "",
                "follower_count": getattr(user, "follower_count", 0) if user else 0,
                "following_count": getattr(user, "following_count", 0) if user else 0,
                "media_count": getattr(user, "media_count", 0) if user else 0,
                "is_private": getattr(account, "is_private", False),
                "is_verified": getattr(account, "is_verified", False),
            }
        )
    except LoginRequired:
        emit_error(
            {"status": "error", "message": "Session expired. Re-run setup.py.", "action": "setup"},
            code=1,
        )
    except Exception as e:
        emit_error({"status": "error", "message": str(e)}, code=2)


def action_get_user_info(client: Client, args: argparse.Namespace) -> None:
    if not args.username:
        emit_error({"status": "error", "message": "--username is required."}, code=3)
    try:
        user = run_sync(client.user_info_by_username, args.username)
        save_session(client)
        emit_ok(
            {
                "status": "ok",
                "action": "get_user_info",
                "username": user.username,
                "full_name": user.full_name,
                "biography": user.biography,
                "follower_count": user.follower_count,
                "following_count": user.following_count,
                "media_count": user.media_count,
                "is_private": user.is_private,
                "is_verified": user.is_verified,
            }
        )
    except LoginRequired:
        emit_error(
            {"status": "error", "message": "Session expired. Re-run setup.py.", "action": "setup"},
            code=1,
        )
    except Exception as e:
        emit_error({"status": "error", "message": str(e)}, code=2)


ACTIONS = {
    "post_photo": action_post_photo,
    "post_reel": action_post_reel,
    "reply_comment": action_reply_comment,
    "add_comment": action_add_comment,
    "like_post": action_like_post,
    "unlike_post": action_unlike_post,
    "follow_user": action_follow_user,
    "unfollow_user": action_unfollow_user,
    "get_feed": action_get_feed,
    "get_user_posts": action_get_user_posts,
    "get_comments": action_get_comments,
    "get_profile": action_get_profile,
    "get_user_info": action_get_user_info,
}


def run_action(client: Client, args: argparse.Namespace) -> None:
    handler = ACTIONS.get(args.action)
    if handler is None:
        emit_error({"status": "error", "message": f"Unknown action: {args.action}"}, code=3)
    handler(client, args)


def main() -> int:
    parser = argparse.ArgumentParser(description="Instagram CLI")
    parser.add_argument(
        "--action",
        required=True,
        choices=sorted(ACTIONS.keys()),
        help="Action to perform",
    )
    parser.add_argument("--file", help="Image/video file path")
    parser.add_argument("--caption", help="Post caption")
    parser.add_argument("--tags", help="Comma-separated hashtags (without #)")
    parser.add_argument("--location", help="Location name (recorded in output only)")
    parser.add_argument("--thumbnail", help="Video thumbnail path (for reels)")
    parser.add_argument("--post-id", help="Post / media ID")
    parser.add_argument("--parent-comment-id", help="Parent comment ID for reply")
    parser.add_argument("--message", help="Comment/reply message")
    parser.add_argument("--username", help="Instagram username")
    parser.add_argument("--limit", type=int, help="Limit for feed/posts")

    args = parser.parse_args()
    client = get_client()
    run_action(client, args)
    return 0


if __name__ == "__main__":
    sys.exit(main())
