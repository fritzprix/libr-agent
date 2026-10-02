---
title: Browser Automation
---

# Browser Automation (Browser Sidecar)

LibrAgent empowers agents to browse the web, search and extract live information, and analyze webpage screenshots autonomously.

---

## 🛡️ Process Isolation & Stability

Complex web pages or in-page script crashes will never compromise your desktop workspace:

- **Isolated Sandbox Execution**: Browser automation operates in an isolated background process separate from the main LibrAgent desktop app. Heavy memory consumption or browser crashes cannot freeze or crash your main application.
- **Sticky agent profile**: By default, `browser__createSession` reuses a fixed LibrAgent agent browser profile on this device. Logins you make *inside* that browser survive later sessions until you clear them in Settings → System → **Agent browser** → Clear agent browser data. This is not your everyday Chrome. Concurrent agent chats share that same cookie jar while the browser sidecar is running.
- **Optional saved browser logins**: Settings → System → **Saved browser logins**. Import from **Chrome, Edge, or Brave** into LibrAgent’s **private copy** (your everyday browser is unchanged — agents do not attach to it). Import may reuse some sessions; for **Google**, if login fails or you see “browser may not be secure”, tap **Open to sign in**, log in once in LibrAgent’s Chrome window, then close it. Agents request that copy with `browser__createSession({ use_profile: true })` (always confirmed; not bypassed by YOLO). Firefox is not supported.

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
