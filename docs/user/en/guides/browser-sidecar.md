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

---

## ✨ New Feature: Everyday Chrome Integration (Stay Logged In)

Instead of opening a blank sandbox browser, agents can directly control tabs in **your personal everyday Chrome browser (`userChrome`)**.

### 💡 Key Benefits for Everyday Users

- **Preserve Logged-in Sessions**: Work directly with services where you are already signed in (Google, GitHub, internal dashboards). You do not need to re-enter passwords.
- **Reliable Clicks on Protected Sites**: Input simulation bypasses strict Content Security Policies (CSP). Button clicks and keystrokes work reliably on complex websites.
- **Save Raw Webpage HTML (`saveRawHtml`)**: Saves complete webpage source files to your session workspace for offline inspection and verification.

| Execution Mode | Behavior | Best Suited For |
| --- | --- | --- |
| **`sidecar`** (Default) | Dedicated isolated agent browser | Public web search, documentation research, sandbox tasks |
| **`userChrome`** (Extension Bridge) | Your personal Chrome browser | Tasks requiring personal logins, private intranets, shopping carts |

---

## 🚀 1-Minute Quick Setup

> [!TIP]
> **Chrome Web Store Release Pending (TBD)**  
> The extension is currently under review for the Chrome Web Store. Until the public store link is published, you can install it locally via the 1-minute `Load unpacked` method below.

1. **Locate Extension Path**: Open **Settings → System → Agent browser** and copy the Chrome extension folder path.
2. **Install Extension in Chrome**:
   - Open `chrome://extensions` in your Chrome browser.
   - Enable **Developer mode** in the top right corner.
   - Click **Load unpacked** and select the extension folder path.
3. **Instruct the Agent**:
   - Prompt the agent naturally in chat:
     ```
     Check the items in my shopping cart in my current Chrome window.
     ```
   - The agent switches to `userChrome` mode and controls your active tab.

> [!NOTE]
> - After installation, the extension reconnects automatically when you restart LibrAgent.
> - Each agent chat maintains exactly one active browser session (Single Source of Truth). Switching between `sidecar` and `userChrome` closes previous sessions cleanly.

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
