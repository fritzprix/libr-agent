# Email browser fallback (Gmail / Outlook web)

Use only after the IMAP script path failed or the user prefers webmail, and only with **explicit confirmation** for `use_profile`.

Hand off procedure details to skill **`browser-session-assist`** (confirm → `createSession` → import / Open to sign in guidance).

There is **no** agent MCP to list profiles. Probe with:

```text
browser__createSession({ "url": "<webmail>", "use_profile": true })
```

Error text containing `No imported browser profile` → guide Settings import (see `browser-session-assist` / its setup-guide).
Error text about saved login **already open** → close the Open-to-sign-in Chrome window, then retry (not the same as Open to sign in for auth).

## When to use

| Trigger | Action |
| --- | --- |
| `python`/`python3` not found | Offer browser fallback |
| Config missing and user skips IMAP setup | Offer browser fallback |
| IMAP/SMTP auth or Modern Auth failure | Offer browser fallback |
| User asks to use logged-in browser mail | Browser path directly (still confirm `use_profile`) |

## Provider entry URLs

| Provider | Start URL |
| --- | --- |
| Gmail | `https://mail.google.com` |
| Outlook / Hotmail / Live | `https://outlook.live.com/mail` |
| Microsoft 365 work/school | `https://outlook.office.com/mail` |

Prefer the URL that matches the user's address. If unknown, ask once or try Gmail then Outlook.

## Supported ops (v1)

| Op | Approach |
| --- | --- |
| Read inbox | Open webmail → `listInteractable` / page content → summarize list |
| Open / read one message | Click the row → extract body; list attachments if visible |
| Search | Use the webmail search box (`inputText` + submit) |
| Compose / send / reply | Open Compose/Reply → fill To/Subject/Body → **confirm with user before Send** |

Mark read / move / delete via UI is **best-effort** only — confirm count and target folder first. Do not claim full IMAP parity.

## Procedure sketch

1. Load `browser-session-assist` rules; get confirmation for saved logins.
2. `browser__createSession({ url: "<webmail>", use_profile: true })`.
3. If not logged in or Google blocks automation → Open to sign in guidance → retry.
4. For the user request: `listInteractable` / `getPageContent` / click / `inputText` as needed.
5. Before sending mail: show draft summary (to, subject, short body preview) and wait for explicit OK.
6. Present results in the same style as IMAP output-format when possible (list then detail).

## Out of scope

- Automating the Settings import UI
- Firefox profiles
- Non-Gmail/Outlook webmail (Naver/Kakao/etc.) in this fallback doc — offer IMAP setup for those, or general `browser-session-assist` if the user already has a logged-in site
