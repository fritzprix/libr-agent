#!/usr/bin/env python3
import argparse
import asyncio
import json
import sys
import re
from pathlib import Path

# --- Twikit Monkey Patches ---
try:
    _tx_mod = __import__('twikit.x_client_transaction.transaction', fromlist=['ClientTransaction'])
    _tx_mod.ON_DEMAND_FILE_REGEX = re.compile(r',(\d+):[\'\"]ondemand\.s[\'\"]')
    _tx_mod.ON_DEMAND_HASH_PATTERN = r',{}:\"([0-9a-f]+)\"'
    
    async def _patched_get_indices(self, home_page_response, session, headers):
        key_byte_indices = []
        response = self.validate_response(home_page_response) or self.home_page_response
        
        match_file = _tx_mod.ON_DEMAND_FILE_REGEX.search(str(response))
        if not match_file:
            raise Exception("Couldn't find ondemand script index on X homepage. X might be blocking request or page format changed.")
            
        on_demand_file_index = match_file.group(1)
        regex = re.compile(_tx_mod.ON_DEMAND_HASH_PATTERN.format(on_demand_file_index))
        match_hash = regex.search(str(response))
        if not match_hash:
            raise Exception("Couldn't find ondemand script hash on X homepage.")
            
        filename = match_hash.group(1)
        on_demand_file_url = f'https://abs.twimg.com/responsive-web/client-web/ondemand.s.{filename}a.js'
        on_demand_file_response = await session.request(method='GET', url=on_demand_file_url, headers=headers)
        
        key_byte_indices_match = _tx_mod.INDICES_REGEX.finditer(str(on_demand_file_response.text))
        for item in key_byte_indices_match:
            key_byte_indices.append(item.group(2))
        
        if not key_byte_indices:
            raise Exception("Couldn't get KEY_BYTE indices from ondemand script.")
        key_byte_indices = list(map(int, key_byte_indices))
        if len(key_byte_indices) < 2:
            raise Exception(
                f"Expected at least 2 KEY_BYTE indices from ondemand script, got {len(key_byte_indices)}."
            )
        return key_byte_indices[0], key_byte_indices[1:]
        
    _tx_mod.ClientTransaction.get_indices = _patched_get_indices
except Exception:
    pass

try:
    from twikit.user import User
    
    class SafeDict(dict):
        def __init__(self, *args, _ctx=None, **kwargs):
            super().__init__(*args, **kwargs)
            self._ctx = _ctx

        def __getitem__(self, key):
            try:
                val = super().__getitem__(key)
                if isinstance(val, dict) and not isinstance(val, SafeDict):
                    child_ctx = 'entities' if self._ctx == 'legacy' and key == 'entities' else self._ctx
                    return SafeDict(val, _ctx=child_ctx)
                return val
            except KeyError:
                if key == 'entities' and self._ctx == 'legacy':
                    return SafeDict(_ctx='entities')
                if self._ctx == 'legacy' and key in ('description', 'url'):
                    return ""
                if self._ctx == 'entities' and key == 'description':
                    return SafeDict({'urls': []}, _ctx='entities')
                if self._ctx == 'entities' and key == 'url':
                    return SafeDict(_ctx='entities')
                if key in ('withheld_in_countries', 'pinned_tweet_ids_str', 'description_urls', 'urls'):
                    return []
                if key in ('possibly_sensitive', 'can_dm', 'can_media_tag', 'want_retweets', 
                           'default_profile', 'default_profile_image', 'has_custom_timelines', 
                           'is_translator', 'protected', 'verified', 'is_blue_verified'):
                    return False
                if key in ('followers_count', 'fast_followers_count', 'normal_followers_count', 
                           'friends_count', 'favourites_count', 'listed_count', 'media_count', 
                           'statuses_count'):
                    return 0
                return ""

        def get(self, key, default=None):
            try:
                return self[key]
            except Exception:
                return default

    _original_user_init = User.__init__
    def _patched_user_init(self, client, data):
        safe_data = SafeDict(data)
        if 'legacy' in safe_data:
            safe_data['legacy'] = SafeDict(safe_data['legacy'], _ctx='legacy')
        _original_user_init(self, client, safe_data)
        
    User.__init__ = _patched_user_init
except Exception:
    pass
# -----------------------------

try:
    from twikit import Client
except ImportError:
    print(json.dumps({"status": "error", "message": "twikit is not installed. Run: pip install twikit"}), file=sys.stderr)
    sys.exit(1)

CONFIG_PATH = Path.home() / ".libragent" / "x_config.json"
COOKIES_PATH = Path.home() / ".libragent" / "x_cookies.json"

def get_client() -> Client:
    if not COOKIES_PATH.exists():
        raise RuntimeError("Not authenticated. Run setup first.")
    
    client = Client('en-US')
    try:
        client.load_cookies(str(COOKIES_PATH))
    except Exception as e:
        raise RuntimeError(f"Failed to load cookies: {e}")
    return client

def safe_int(val) -> int:
    if val is None:
        return 0
    if isinstance(val, int):
        return val
    try:
        val_str = str(val).strip().upper()
        if not val_str:
            return 0
        multiplier = 1
        if val_str.endswith('K'):
            multiplier = 1000
            val_str = val_str[:-1]
        elif val_str.endswith('M'):
            multiplier = 1000000
            val_str = val_str[:-1]
        val_str = val_str.replace(',', '')
        return int(float(val_str) * multiplier)
    except (ValueError, TypeError):
        return 0

def format_tweet(tweet) -> dict:
    # Defensive: tweet.user can be None, a dict, or an object
    user = getattr(tweet, 'user', None)
    if isinstance(user, dict):
        screen_name = user.get('screen_name', user.get('name', 'unknown'))
        user_name = user.get('name', screen_name)
    elif user is not None:
        screen_name = getattr(user, 'screen_name', 'unknown')
        user_name = getattr(user, 'name', screen_name)
    else:
        screen_name = 'unknown'
        user_name = 'unknown'

    created_at = None
    if hasattr(tweet, 'created_at'):
        created_at = tweet.created_at
    elif hasattr(tweet, 'date'):
        created_at = tweet.date

    return {
        "id": str(getattr(tweet, 'id', 'unknown')),
        "created_at": str(created_at) if created_at else None,
        "text": getattr(tweet, 'text', ''),
        "user": screen_name,
        "user_name": user_name,
        "likes": safe_int(getattr(tweet, 'favorite_count', getattr(tweet, 'favoriteCount', 0))),
        "retweets": safe_int(getattr(tweet, 'retweet_count', getattr(tweet, 'retweetCount', 0))),
        "reply_count": safe_int(getattr(tweet, 'reply_count', getattr(tweet, 'replyCount', 0))),
        "quote_count": safe_int(getattr(tweet, 'quote_count', getattr(tweet, 'quoteCount', 0))),
        "views": safe_int(getattr(tweet, 'impression_count', getattr(tweet, 'views', 0))),
    }

async def run_cli(args) -> int:
    try:
        client = get_client()

        if args.action == "post_tweet":
            if not args.message:
                print(json.dumps({"status": "error", "message": "Message is required to post a tweet."}), file=sys.stderr)
                return 3
            
            media_ids = []
            if args.file:
                media_path = Path(args.file)
                if not media_path.exists():
                    print(json.dumps({"status": "error", "message": f"File not found: {args.file}"}), file=sys.stderr)
                    return 3
                # Upload media
                media_id = await client.upload_media(str(media_path))
                media_ids.append(media_id)
            
            tweet = await client.create_tweet(text=args.message, media_ids=media_ids if media_ids else None)
            print(json.dumps({
                "status": "ok",
                "tweet_id": tweet.id,
                "message": "Tweet posted successfully."
            }))
            return 0

        elif args.action == "get_timeline":
            tweets = await client.get_latest_timeline(count=args.limit)
            formatted = []
            for t in tweets:
                try:
                    formatted.append(format_tweet(t))
                except Exception as _e:
                    print(f"[WARN] Skipping malformed tweet: {_e}", file=sys.stderr)
                    continue
            print(json.dumps({
                "status": "ok",
                "count": len(formatted),
                "tweets": formatted
            }, ensure_ascii=False))
            return 0

        elif args.action == "get_user_tweets":
            if not args.username:
                print(json.dumps({"status": "error", "message": "Username is required."}), file=sys.stderr)
                return 3
            user = await client.get_user_by_screen_name(args.username)
            tweets = await user.get_tweets('Tweets', count=args.limit)
            formatted = [format_tweet(t) for t in tweets]
            print(json.dumps({
                "status": "ok",
                "username": args.username,
                "count": len(formatted),
                "tweets": formatted
            }, ensure_ascii=False))
            return 0

        elif args.action == "search_tweets":
            if not args.query:
                print(json.dumps({"status": "error", "message": "Query is required."}), file=sys.stderr)
                return 3
            search_count = min(max(args.limit, 1), 20)
            tweets = await client.search_tweet(args.query, args.product, count=search_count)
            formatted = [format_tweet(t) for t in tweets]
            print(json.dumps({
                "status": "ok",
                "query": args.query,
                "product": args.product,
                "count": len(formatted),
                "tweets": formatted
            }, ensure_ascii=False))
            return 0

        elif args.action == "like_tweet":
            if not args.tweet_id:
                print(json.dumps({"status": "error", "message": "Tweet ID is required."}), file=sys.stderr)
                return 3
            await client.favorite_tweet(args.tweet_id)
            print(json.dumps({
                "status": "ok",
                "tweet_id": args.tweet_id,
                "message": "Tweet liked successfully."
            }))
            return 0

        elif args.action == "retweet":
            if not args.tweet_id:
                print(json.dumps({"status": "error", "message": "Tweet ID is required."}), file=sys.stderr)
                return 3
            await client.retweet(args.tweet_id)
            print(json.dumps({
                "status": "ok",
                "tweet_id": args.tweet_id,
                "message": "Retweeted successfully."
            }))
            return 0

    except Exception as e:
        print(json.dumps({"status": "error", "message": str(e)}), file=sys.stderr)
        return 2

def main() -> int:
    parser = argparse.ArgumentParser(description="X CLI client")
    parser.add_argument("--action", required=True, choices=[
        "post_tweet", "get_timeline", "get_user_tweets", "search_tweets", "like_tweet", "retweet"
    ])
    parser.add_argument("--message", help="Tweet message content")
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

    args = parser.parse_args()
    return asyncio.run(run_cli(args))

if __name__ == "__main__":
    sys.exit(main())
