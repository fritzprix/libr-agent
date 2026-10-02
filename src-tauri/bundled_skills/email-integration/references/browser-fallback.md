# Email browser fallback (Gmail / Outlook web)

Use only after the IMAP script path failed or the user prefers webmail.

Uses LibrAgent’s **sticky agent browser profile** — logins made inside that browser survive later `createSession` until cleared in Settings → System → Agent browser. This is not everyday Chrome.

```text
browser__createSession({ "url": "<webmail>" })
```

If the page shows a login wall, ask the user to sign in inside the agent browser window, then continue.

## When to use

| Trigger | Action |
| --- | --- |
| `python`/`python3` not found | Offer browser fallback |
| Config missing and user skips IMAP setup | Offer browser fallback |
| IMAP/SMTP auth or Modern Auth failure | Offer browser fallback |
| User asks to use logged-in browser mail | Browser path directly |

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

1. `browser__createSession({ url: "<webmail>" })`.
2. If not logged in → ask user to sign in in the agent browser, then retry or continue.
3. For the user request: `listInteractable` / `getPageContent` / click / `inputText` as needed.
4. Before sending mail: show draft summary (to, subject, short body preview) and wait for explicit OK.
5. Present results in the same style as IMAP output-format when possible (list then detail).

## Out of scope

- Importing everyday Chrome/Edge/Brave profiles into LibrAgent
- Firefox profiles
- Non-Gmail/Outlook webmail (Naver/Kakao/etc.) in this fallback doc — offer IMAP setup for those, or a general sticky-browser session if the user already has a logged-in site
