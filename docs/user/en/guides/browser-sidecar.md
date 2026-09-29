---
title: Browser Automation
---

# Browser Automation (Browser Sidecar)

LibrAgent empowers agents to browse the web, search and extract live information, and analyze webpage screenshots autonomously.

---

## 🛡️ Process Isolation & Stability

Complex web pages or in-page script crashes will never compromise your desktop workspace:

- **Isolated Sandbox Execution**: Browser automation operates in an isolated background process separate from the main LibrAgent desktop app. Heavy memory consumption or browser crashes cannot freeze or crash your main application.
- **Privacy Protection**: Uses a dedicated, clean browser profile completely isolated from your personal browser cookies, history, and login sessions by default.
- **Optional saved browser logins**: Settings → System → **Saved browser logins**. Close Chrome/Edge/Brave/Firefox completely (including the tray icon near the clock), then tap **Import from my browsers**. LibrAgent copies signed-in sessions into its own folder; your real browsers are unchanged. Agents may request them with `browser__createSession({ use_profile: true })`, which always asks for your confirmation (not bypassed by YOLO). (Internally this is stored as an imported browser profile — the Settings UI says “logins” on purpose.)

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
