---
title: Browser Automation
---

# Browser Automation (Browser Sidecar)

LibrAgent empowers agents to browse the web, search and extract live information, and analyze webpage screenshots autonomously.

---

## 🛡️ Process Isolation & Stability

Complex web pages or in-page script crashes will never compromise your desktop workspace:

- **Isolated Sandbox Execution**: Browser automation operates in an isolated background process separate from the main LibrAgent desktop app. Heavy memory consumption or browser crashes cannot freeze or crash your main application.
- **Sticky agent profile**: `browser__createSession` with `browser="sidecar"` (default) reuses a fixed LibrAgent agent browser profile on this device. Logins you make *inside* that browser survive later sessions until you clear them in Settings → System → **Agent browser** → Clear agent browser data. This is not your everyday Chrome. Concurrent agent chats share that same cookie jar while the browser sidecar is running.

---

## Everyday Chrome (MV3 extension bridge)

For logged-in sites in **your everyday Chrome**, LibrAgent can optionally drive tabs through a local MV3 extension (Load unpacked or Chrome Web Store) and a loopback WebSocket bridge (`ws://127.0.0.1:3847/extension-bridge` by default).

| `browser` on `createSession` | Meaning |
| --- | --- |
| **`sidecar`** (default) | Sticky CDP agent browser — separate from everyday Chrome |
| **`userChrome`** | Everyday Chrome via the extension bridge — **errors if not Connected**; never silently opens sidecar |

Agents must pick the target at create time. A failed `userChrome` session does not contaminate reads by falling back to the agent browser.

**One active browser session per agent (SSOT):** each agent chat has a single active browser session. `browser__createSession` replaces any previous session (including when switching `sidecar` ↔ `userChrome`). Other browser tools always target that active session — there is no `sessionId` argument and no dual-open of everyday Chrome + agent browser at once. To change backend, create again.

- Install steps: see `chrome-extension/README.md`, or copy the path from Settings → System → **Agent browser** → Chrome extension bridge (status-first card). Chrome Web Store packaging notes live in `chrome-extension/STORE.md`.
- After a one-time install (Load unpacked or Store), LibrAgent restarts should reconnect automatically. Use the toolbar popup or Settings status — you only need **Reload** on `chrome://extensions` when the extension files themselves change (developer updates).
- With `browser="userChrome"` and Connected, most browser tools (navigate, content, click/input, evaluateJS, viewport screenshot) use everyday Chrome. `getConsoleLogs` remains sticky-sidecar only. Extension screenshots are viewport-only (`fullPage` ignored).
- Auth: fixed dev token `libragent-dev` when `LIBRAGENT_EXTENSION_BRIDGE_TOKEN` is unset. Production will use a rotating token.

---

## 🌐 Key Capabilities

| Capability                                                              | Description                                                       |
| ----------------------------------------------------------------------- | ----------------------------------------------------------------- |
| **Page Navigation** (`browser__navigateToUrl`)                          | Visits specified URLs and detects page readiness to read content. |
| **Screenshot Capture** (`browser__takeScreenshot`)                      | Captures viewport or full-page images for visual inspection.      |
| **Console & Error Tracking**                                            | Monitors page JavaScript errors and network state.                |
| **Interaction Support** (`browser__clickElement`, `browser__inputText`) | Supports clicking links, typing text, and scrolling pages.        |

---

## 🎯 Practical Use Cases

1. **Research Live Documentation**:
   - Gathers live release notes, API updates, or competitive news from real web pages.
2. **Web UI Verification**:
   - Visits local development servers (`http://localhost:3000`) and captures screenshots to visually verify UI layouts.
3. **Automated Daily Briefings**:
   - Pairs with Scheduled Tasks to visit specified news sites or dashboards every morning and generate summaries.

---

## 💡 Notes

- Initial browser launch may take a few seconds while the headless runtime prepares.
