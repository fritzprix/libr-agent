#!/usr/bin/env python3
import argparse
import asyncio
import json
import os
import sys
from pathlib import Path

if os.name == "nt" and sys.stdout.encoding and sys.stdout.encoding.lower() != "utf-8":
    sys.stdout.reconfigure(encoding="utf-8")

# Ensure local scripts/ is importable when invoked by absolute path.
_SCRIPTS_DIR = Path(__file__).resolve().parent
if str(_SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS_DIR))

from twikit_patches import apply_twikit_patches  # noqa: E402

apply_twikit_patches()

try:
    from twikit import Client
except ImportError:
    print(
        json.dumps(
            {"status": "error", "message": "twikit is not installed. Run: pip install twikit"}
        ),
        file=sys.stderr,
    )
    sys.exit(1)

CONFIG_PATH = Path.home() / ".libragent" / "x_config.json"
COOKIES_PATH = Path.home() / ".libragent" / "x_cookies.json"
# Tweet/thread text via --message-file; generous headroom above X limits.
MAX_MESSAGE_FILE_BYTES = 64 * 1024
OP_TIMEOUT_SEC = 60


async def await_op(awaitable):
    """Await a Twikit coroutine with a hard timeout."""
    try:
        return await asyncio.wait_for(awaitable, timeout=OP_TIMEOUT_SEC)
    except asyncio.TimeoutError as exc:
        raise TimeoutError(f"X operation timed out after {OP_TIMEOUT_SEC}s") from exc


def get_client() -> Client:
    if not COOKIES_PATH.exists():
        raise RuntimeError("Not authenticated. Run setup first.")

    client = Client("en-US")
    try:
        client.load_cookies(str(COOKIES_PATH))
    except Exception as e:
        raise RuntimeError(f"Failed to load cookies: {e}") from e
    return client


def read_message_file(file_path: str) -> str | None:
    """Read tweet text from a UTF-8 file, avoiding shell quoting/expansion issues."""
    path = Path(file_path)
    if not path.exists():
        print(
            json.dumps({"status": "error", "message": f"Message file not found: {file_path}"}),
            file=sys.stderr,
        )
        return None
    try:
        file_size = path.stat().st_size
    except OSError as e:
        print(
            json.dumps({"status": "error", "message": f"Failed to access message file: {e}"}),
            file=sys.stderr,
        )
        return None

    if file_size > MAX_MESSAGE_FILE_BYTES:
        print(
            json.dumps(
                {
                    "status": "error",
                    "message": (
                        f"Message file exceeds the limit of {MAX_MESSAGE_FILE_BYTES} bytes "
                        f"(got {file_size} bytes)."
                    ),
                }
            ),
            file=sys.stderr,
        )
        return None

    try:
        content = path.read_text(encoding="utf-8").strip()
    except OSError as e:
        print(
            json.dumps({"status": "error", "message": f"Failed to read message file: {e}"}),
            file=sys.stderr,
        )
        return None

    if not content:
        print(
            json.dumps(
                {
                    "status": "error",
                    "message": "Message file is empty after stripping whitespace.",
                }
            ),
            file=sys.stderr,
        )
        return None

    return content


def resolve_message(args) -> str | None:
    if args.message_file:
        return read_message_file(args.message_file)
    return args.message


def safe_int(val) -> int:
    if val is None:
        return 0
    if isinstance(val, int):
        return val
    try:
        return int(str(val).replace(",", "").strip() or 0)
    except (ValueError, TypeError):
        return 0


def format_tweet(tweet) -> dict:
    # Defensive: tweet.user can be None, a dict, or an object
    user = getattr(tweet, "user", None)
    if isinstance(user, dict):
        screen_name = user.get("screen_name", user.get("name", "unknown"))
        user_name = user.get("name", screen_name)
    elif user is not None:
        screen_name = getattr(user, "screen_name", "unknown")
        user_name = getattr(user, "name", screen_name)
    else:
        screen_name = "unknown"
        user_name = "unknown"

    created_at = None
    if hasattr(tweet, "created_at"):
        created_at = tweet.created_at
    elif hasattr(tweet, "date"):
        created_at = tweet.date

    return {
        "id": str(getattr(tweet, "id", "unknown")),
        "created_at": str(created_at) if created_at else None,
        "text": getattr(tweet, "text", ""),
        "user": screen_name,
        "user_name": user_name,
        "likes": safe_int(getattr(tweet, "favorite_count", getattr(tweet, "favoriteCount", 0))),
        "retweets": safe_int(getattr(tweet, "retweet_count", getattr(tweet, "retweetCount", 0))),
        "reply_count": safe_int(getattr(tweet, "reply_count", getattr(tweet, "replyCount", 0))),
        "quote_count": safe_int(getattr(tweet, "quote_count", getattr(tweet, "quoteCount", 0))),
        "views": safe_int(getattr(tweet, "impression_count", getattr(tweet, "views", 0))),
    }


async def run_cli(args) -> int:
    try:
        client = get_client()

        if args.action == "post_tweet":
            message = resolve_message(args)
            if not message:
                if not args.message and not args.message_file:
                    print(
                        json.dumps(
                            {
                                "status": "error",
                                "message": (
                                    "Message is required to post a tweet. "
                                    "Use --message or --message-file."
                                ),
                            }
                        ),
                        file=sys.stderr,
                    )
                return 3

            media_ids = []
            if args.file:
                media_path = Path(args.file)
                if not media_path.exists():
                    print(
                        json.dumps(
                            {"status": "error", "message": f"File not found: {args.file}"}
                        ),
                        file=sys.stderr,
                    )
                    return 3
                media_id = await await_op(client.upload_media(str(media_path)))
                media_ids.append(media_id)

            tweet = await await_op(
                client.create_tweet(
                    text=message,
                    media_ids=media_ids if media_ids else None,
                    reply_to=args.reply_to,
                )
            )
            print(
                json.dumps(
                    {
                        "status": "ok",
                        "tweet_id": tweet.id,
                        "reply_to": args.reply_to,
                        "message": (
                            "Reply posted successfully."
                            if args.reply_to
                            else "Tweet posted successfully."
                        ),
                    }
                )
            )
            return 0

        if args.action == "get_timeline":
            tweets = await await_op(client.get_latest_timeline(count=args.limit))
            formatted = []
            for t in tweets:
                try:
                    formatted.append(format_tweet(t))
                except Exception as _e:
                    print(f"[WARN] Skipping malformed tweet: {_e}", file=sys.stderr)
                    continue
            print(
                json.dumps(
                    {"status": "ok", "count": len(formatted), "tweets": formatted},
                    ensure_ascii=False,
                )
            )
            return 0

        if args.action == "get_user_tweets":
            if not args.username:
                print(
                    json.dumps({"status": "error", "message": "Username is required."}),
                    file=sys.stderr,
                )
                return 3
            user = await await_op(client.get_user_by_screen_name(args.username))
            tweets = await await_op(user.get_tweets("Tweets", count=args.limit))
            formatted = [format_tweet(t) for t in tweets]
            print(
                json.dumps(
                    {
                        "status": "ok",
                        "username": args.username,
                        "count": len(formatted),
                        "tweets": formatted,
                    },
                    ensure_ascii=False,
                )
            )
            return 0

        if args.action == "search_tweets":
            if not args.query:
                print(
                    json.dumps({"status": "error", "message": "Query is required."}),
                    file=sys.stderr,
                )
                return 3
            search_count = min(max(args.limit, 1), 20)
            tweets = await await_op(
                client.search_tweet(args.query, args.product, count=search_count)
            )
            formatted = [format_tweet(t) for t in tweets]
            print(
                json.dumps(
                    {
                        "status": "ok",
                        "query": args.query,
                        "product": args.product,
                        "count": len(formatted),
                        "tweets": formatted,
                    },
                    ensure_ascii=False,
                )
            )
            return 0

        if args.action == "like_tweet":
            if not args.tweet_id:
                print(
                    json.dumps({"status": "error", "message": "Tweet ID is required."}),
                    file=sys.stderr,
                )
                return 3
            await await_op(client.favorite_tweet(args.tweet_id))
            print(
                json.dumps(
                    {
                        "status": "ok",
                        "tweet_id": args.tweet_id,
                        "message": "Tweet liked successfully.",
                    }
                )
            )
            return 0

        if args.action == "retweet":
            if not args.tweet_id:
                print(
                    json.dumps({"status": "error", "message": "Tweet ID is required."}),
                    file=sys.stderr,
                )
                return 3
            await await_op(client.retweet(args.tweet_id))
            print(
                json.dumps(
                    {
                        "status": "ok",
                        "tweet_id": args.tweet_id,
                        "message": "Retweeted successfully.",
                    }
                )
            )
            return 0

        if args.action == "delete_tweet":
            if not args.tweet_id:
                print(
                    json.dumps({"status": "error", "message": "Tweet ID is required."}),
                    file=sys.stderr,
                )
                return 3
            await await_op(client.delete_tweet(args.tweet_id))
            print(
                json.dumps(
                    {
                        "status": "ok",
                        "tweet_id": args.tweet_id,
                        "message": "Tweet deleted successfully.",
                    }
                )
            )
            return 0

        print(
            json.dumps({"status": "error", "message": f"Unknown action: {args.action}"}),
            file=sys.stderr,
        )
        return 3

    except TimeoutError as e:
        print(json.dumps({"status": "error", "message": str(e)}), file=sys.stderr)
        return 2
    except Exception as e:
        print(json.dumps({"status": "error", "message": str(e)}), file=sys.stderr)
        return 2


def main() -> int:
    parser = argparse.ArgumentParser(description="X CLI client")
    parser.add_argument(
        "--action",
        required=True,
        choices=[
            "post_tweet",
            "get_timeline",
            "get_user_tweets",
            "search_tweets",
            "like_tweet",
            "retweet",
            "delete_tweet",
        ],
    )
    parser.add_argument("--message", help="Tweet message content")
    parser.add_argument(
        "--message-file",
        help=(
            "Read tweet text from a UTF-8 file "
            "(recommended when the message contains $ or shell-special characters)"
        ),
    )
    parser.add_argument("--file", help="Path to image/video attachment")
    parser.add_argument("--limit", type=int, default=10, help="Number of tweets to retrieve")
    parser.add_argument("--username", help="X username/screen_name of target user")
    parser.add_argument("--query", help="Search query")
    parser.add_argument(
        "--product",
        choices=["Top", "Latest", "Media"],
        default="Top",
        help="Search result type for search_tweets (default: Top)",
    )
    parser.add_argument("--tweet-id", help="Target tweet ID")
    parser.add_argument(
        "--reply-to",
        "--reply_to",
        dest="reply_to",
        help="Tweet ID to reply to (creates a thread reply when posting)",
    )

    args = parser.parse_args()
    return asyncio.run(run_cli(args))


if __name__ == "__main__":
    sys.exit(main())
