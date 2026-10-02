---
name: browser-session-assist
description: |
  Guide users through LibrAgent saved browser logins and run browser tools with use_profile=true.
  Use when the user needs their already-logged-in Chromium copy (Gmail, Outlook, Google, banking, etc.),
  when IMAP/CLI auth failed and web session is the fallback, when createSession fails with no imported
  profile, or when Google shows "browser may not be secure".
  Triggers: "브라우저 로그인", "saved browser logins", "use_profile", "Open to sign in",
  "프로필 가져오기", "저장된 브라우저 로그인", "웹메일로 열어줘", "가져오기 시작".
---

# Browser Session Assist

Help the agent use the user's **LibrAgent saved browser logins** (app-local Chromium copy), not their everyday Chrome/Edge window.

Other skills (e.g. `email-integration`) may hand off here after script/API auth fails.

## Mental model (tell the user this way)

1. **Start import…** — guided **4 steps** (choose → close browsers → copy → done). Not one click; closing is required so login files unlock.
2. **Google still asks you to sign in?** — rare recovery under an already-saved login. Open LibrAgent-only Chrome, sign in, then **I’m done signing in**.
3. Never say “keep the login window open” for agent attach. That is wrong.

## Security (mandatory)

- **Always** get **explicit user confirmation** before `browser__createSession({ use_profile: true })`. Not bypassed by YOLO.
- The UI will also show a **hard approval** for `use_profile=true`. Chat OK does **not** replace that prompt — do not re-ask the same question after the user already approved in the UI.
- Pass **`use_profile` as a boolean only**. Never invent or request filesystem profile paths.
- Saved logins are an **app-local Chromium copy** — not the user's daily browser. **Firefox is not supported.**
- Prefer Settings recovery (**Google still asks…** → open → **I’m done signing in**) when Google blocks automation login.
- Do not scrape or paste passwords from the page into chat.

## Workflow

### 1. Confirm

Ask once: use the LibrAgent **Saved browser logins** profile for this task? Proceed only if the user agrees. Expect a second confirmation in the product UI when the tool runs.

### 2. Open profile session

```text
browser__createSession({ "url": "<target or https://www.google.com>", "use_profile": true })
```

There is no MCP to list profiles — probe by creating the session.

| Result | Next step |
| --- | --- |
| Session OK | Continue with `browser__navigateToUrl` / `browser__listInteractable` / click / input as needed |
| Error contains `No imported browser profile` | Step 3a — guide import wizard |
| Error contains `already open` / `Close login window` / saved login window still open | Step 3c — close window, then retry (do **not** treat as sign-in) |
| Google “browser may not be secure”, logged-out, or auth wall | Step 3b — Google recovery, **I’m done**, then retry |
| Profile-mode switch error | Retry `createSession` once (leftover sessions recycle on retry) |

### 3a. Guide import wizard (no profile)

Tell the user (do not automate Settings UI):

1. **Settings → System → Saved browser logins → Start import…**
2. Follow the four steps: **choose** browsers → **close** them → **copy** → **done**
3. Optionally set the default profile for agents afterward
4. User says when done → retry step 2
5. Only if still logged out / Google blocked: Step 3b

Details: [references/setup-guide.md](references/setup-guide.md).

### 3b. Guide Google recovery (profile exists, not logged in)

1. Settings → System → Saved browser logins → expand **Google still asks you to sign in?**
2. **Open Chrome to sign in** → complete login in the LibrAgent Chrome window
3. Tap **I’m done signing in** — **required**
4. Confirm in chat, then retry step 2 with the same target URL

**Never** tell the user to keep that window open for the agent.

### 3c. Profile already in use

If the error says the saved login is **already open** / close the login window:

1. Ask the user to expand the Google recovery row and tap **I’m done signing in**, or close that LibrAgent Chrome themselves
2. Do **not** send them through Open again unless they are also logged out
3. Retry step 2

### 4. Operate

After a working profile session, use normal browser tools. Prefer the user's destination URL in `createSession` when known (e.g. `https://mail.google.com`).

## Related

- `email-integration` — IMAP first; falls back to this skill for Gmail/Outlook web
- User guide: `docs/user/en/guides/browser-sidecar.md` (KO: `docs/user/guides/browser-sidecar.md`) — Saved browser logins / optional profile sessions
