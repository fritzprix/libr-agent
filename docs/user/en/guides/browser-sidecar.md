---
title: Browser automation
description: 'When to use the built-in agent browser versus your everyday Chrome.'
---

# Browser automation

LibrAgent agents can open web pages, read content, and capture screenshots.

---

## Built-in agent browser (default)

By default, LibrAgent opens a **dedicated agent browser**.

- It runs in a separate process, so a heavy or broken page does not freeze the main app.
- Logins you make _inside_ that browser stay on this device until you clear them under **Settings → System → Agent browser → Clear agent browser data**.
- This profile is not your everyday Chrome.

Use it for public search, docs, and sandbox work.

---

## Your Chrome (only when you need it)

Install the Chrome extension when the agent must use sites where **you are already signed in** — carts, mail, intranet, personal dashboards.

|          | Built-in agent browser | Your Chrome                              |
| -------- | ---------------------- | ---------------------------------------- |
| Logins   | LibrAgent-only profile | Your normal Chrome cookies               |
| Best for | Public research        | Personal or work sites with your session |
| Setup    | None                   | One Chrome extension install             |

### Install from the Chrome Web Store

1. Start the LibrAgent desktop app.
2. In Chrome, install **[LibrAgent Browser Bridge](https://chromewebstore.google.com/detail/libragent-browser-bridge/eponcjengahlpkgbaolhemhmckopmfbe)**.
3. Open the toolbar icon and wait until the status shows **Connected**.  
   If you see “LibrAgent not running”, start the app and wait a few seconds.
4. Ask in chat, for example:
   ```
   Check my shopping cart in the Chrome window I’m using.
   ```

**Settings → System → Your Chrome** also has an Install from Chrome Web Store button.

### Load from a local folder (developers)

Copy the path from Settings, then in Chrome open `chrome://extensions`, turn on Developer mode, and choose **Load unpacked**.

> [!NOTE]
>
> - After install, the extension reconnects when you restart LibrAgent.
> - Each chat keeps one active browser. Switching modes closes the previous link.
> - The extension only links LibrAgent and Chrome **on this computer**. It does not upload your browsing data to a LibrAgent cloud by itself.

---

## What you can do

| Capability                  | Built-in                       | Your Chrome                              |
| --------------------------- | ------------------------------ | ---------------------------------------- |
| Open and move between pages | ✅                             | ✅                                       |
| Read page content           | ✅                             | ✅                                       |
| Click, type, scroll         | ✅                             | ✅ (works on many strict-security sites) |
| Screenshot                  | ✅ (visible area or full page) | ✅ (visible area only)                   |
| Console logs                | ✅                             | ❌ (not yet)                             |
| Use your Chrome logins      | ❌                             | ✅                                       |

Typical “Your Chrome” tasks: check logged-in shopping, mail, or GitHub; open intranet dashboards; capture the visible tab; click and type in forms.

---

## Example uses

1. **Live docs and news** — Open public pages and summarize them.
2. **Local UI checks** — Open `http://localhost:3000` and capture the screen.
3. **Scheduled briefings** — Pair with Scheduled Tasks to visit the same sites each day.

---

## Note

- The first launch of the built-in browser may take a few seconds while the runtime starts.
