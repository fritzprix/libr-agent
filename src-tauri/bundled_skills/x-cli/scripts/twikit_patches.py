"""Shared Twikit compatibility patches for x-cli scripts.

Applied once per process. Safe to import from both setup.py and x_cli.py.
"""

from __future__ import annotations

import re
from typing import Any


_APPLIED = False


def apply_twikit_patches() -> None:
    """Patch Twikit internals used by X homepage transaction parsing.

    Current Twikit (2.3.x) exposes async ``ClientTransaction.get_indices``;
    our replacement stays async. Failures are swallowed so scripts can still
    surface a clear import/runtime error later.
    """
    global _APPLIED
    if _APPLIED:
        return
    _APPLIED = True

    try:
        import twikit

        version = getattr(twikit, "__version__", "0")
        major_s = str(version).split(".", 1)[0]
        if major_s.isdigit() and int(major_s) >= 3:
            # Pin is twikit>=2.3.3,<3 — warn-compatible only.
            pass
    except Exception:
        pass

    try:
        _tx_mod = __import__(
            "twikit.x_client_transaction.transaction",
            fromlist=["ClientTransaction"],
        )
        _tx_mod.ON_DEMAND_FILE_REGEX = re.compile(r",(\d+):['\"]ondemand\.s['\"]")
        _tx_mod.ON_DEMAND_HASH_PATTERN = r',{}:"([0-9a-f]+)"'

        async def _patched_get_indices(
            self: Any,
            home_page_response: Any,
            session: Any,
            headers: Any,
        ) -> tuple[int, list[int]]:
            key_byte_indices: list[Any] = []
            response = self.validate_response(home_page_response) or self.home_page_response

            match_file = _tx_mod.ON_DEMAND_FILE_REGEX.search(str(response))
            if not match_file:
                raise Exception(
                    "Couldn't find ondemand script index on X homepage. "
                    "X might be blocking request or page format changed."
                )

            on_demand_file_index = match_file.group(1)
            regex = re.compile(_tx_mod.ON_DEMAND_HASH_PATTERN.format(on_demand_file_index))
            match_hash = regex.search(str(response))
            if not match_hash:
                raise Exception("Couldn't find ondemand script hash on X homepage.")

            filename = match_hash.group(1)
            on_demand_file_url = (
                f"https://abs.twimg.com/responsive-web/client-web/ondemand.s.{filename}a.js"
            )
            on_demand_file_response = await session.request(
                method="GET",
                url=on_demand_file_url,
                headers=headers,
            )

            key_byte_indices_match = _tx_mod.INDICES_REGEX.finditer(
                str(on_demand_file_response.text)
            )
            for item in key_byte_indices_match:
                key_byte_indices.append(item.group(2))

            if not key_byte_indices:
                raise Exception("Couldn't get KEY_BYTE indices from ondemand script.")
            key_byte_indices_int = list(map(int, key_byte_indices))
            if len(key_byte_indices_int) < 2:
                raise Exception(
                    "Expected at least 2 KEY_BYTE indices from ondemand script, "
                    f"got {len(key_byte_indices_int)}."
                )
            return key_byte_indices_int[0], key_byte_indices_int[1:]

        _tx_mod.ClientTransaction.get_indices = _patched_get_indices
    except Exception:
        pass

    try:
        from twikit.user import User

        class SafeDict(dict):
            def __init__(self, *args: Any, _ctx: Any = None, **kwargs: Any) -> None:
                super().__init__(*args, **kwargs)
                self._ctx = _ctx

            def __getitem__(self, key: Any) -> Any:
                try:
                    val = super().__getitem__(key)
                    if isinstance(val, dict) and not isinstance(val, SafeDict):
                        child_ctx = (
                            "entities"
                            if self._ctx == "legacy" and key == "entities"
                            else self._ctx
                        )
                        return SafeDict(val, _ctx=child_ctx)
                    return val
                except KeyError:
                    if key == "entities" and self._ctx == "legacy":
                        return SafeDict(_ctx="entities")
                    if self._ctx == "legacy" and key in ("description", "url"):
                        return ""
                    if self._ctx == "entities" and key == "description":
                        return SafeDict({"urls": []}, _ctx="entities")
                    if self._ctx == "entities" and key == "url":
                        return SafeDict(_ctx="entities")
                    if key in (
                        "withheld_in_countries",
                        "pinned_tweet_ids_str",
                        "description_urls",
                        "urls",
                    ):
                        return []
                    if key in (
                        "possibly_sensitive",
                        "can_dm",
                        "can_media_tag",
                        "want_retweets",
                        "default_profile",
                        "default_profile_image",
                        "has_custom_timelines",
                        "is_translator",
                        "protected",
                        "verified",
                        "is_blue_verified",
                    ):
                        return False
                    if key in (
                        "followers_count",
                        "fast_followers_count",
                        "normal_followers_count",
                        "friends_count",
                        "favourites_count",
                        "listed_count",
                        "media_count",
                        "statuses_count",
                    ):
                        return 0
                    return ""

            def get(self, key: Any, default: Any = None) -> Any:
                try:
                    return self[key]
                except Exception:
                    return default

        _original_user_init = User.__init__

        def _patched_user_init(self: Any, client: Any, data: Any) -> None:
            safe_data = SafeDict(data)
            if "legacy" in safe_data:
                safe_data["legacy"] = SafeDict(safe_data["legacy"], _ctx="legacy")
            _original_user_init(self, client, safe_data)

        User.__init__ = _patched_user_init
    except Exception:
        pass
