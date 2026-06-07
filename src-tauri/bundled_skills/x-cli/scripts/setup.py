#!/usr/bin/env python3
import argparse
import asyncio
from getpass import getpass
import json
import re
import sys
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
            # Set and validate cookies directly
            cookies = {
                "auth_token": auth_token,
                "ct0": ct0
            }
            client.set_cookies(cookies)
            
            # Validate cookies by attempting to fetch a timeline entry
            await client.get_latest_timeline(count=1)
            
            # Save valid cookies
            client.save_cookies(str(COOKIES_PATH))
        else:
            if not password:
                print(json.dumps({"status": "error", "message": "Password is required for credentials login."}), file=sys.stderr)
                return 3
            
            # Perform login
            await client.login(
                auth_info_1=args.username,
                auth_info_2=args.email,
                password=password,
                totp_secret=args.totp_secret
            )
            
            # Save cookies
            client.save_cookies(str(COOKIES_PATH))

        # Save config
        config_data = {
            "username": args.username,
            "email": args.email
        }
        CONFIG_PATH.write_text(json.dumps(config_data, indent=2), encoding="utf-8")

        print(json.dumps({
            "status": "ok",
            "username": args.username,
            "message": "Authentication successful. Session cookies saved."
        }))
        return 0

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
        password = sys.stdin.readline().strip()

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
