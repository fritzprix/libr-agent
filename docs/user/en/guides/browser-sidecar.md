---
title: Browser Automation
description: 'Configure the dedicated agent browser and Personal Chrome for login-aware automation.'
---

# Browser Automation (Browser Sidecar)

LibrAgent empowers agents to browse the web, search and extract live information, and analyze webpage screenshots autonomously.

---

## 🛡️ Process Isolation & Stability

Complex web pages or in-page script crashes will never compromise your desktop workspace:

- **Isolated Sandbox Execution**: Browser automation operates in an isolated background process separate from the main LibrAgent desktop app. Heavy memory consumption or browser crashes cannot freeze or crash your main application.
- **Sticky agent profile**: By default, the **Dedicated agent browser** reuses a fixed LibrAgent browser profile on this device. Logins you make _inside_ that browser survive later sessions until you clear them in **Settings → System → Agent browser → Clear agent browser data**. This is not your everyday Chrome. Concurrent agent chats share that same cookie jar while the dedicated agent browser is running.

---

## Everyday Chrome Integration (Stay Logged In)

Instead of opening a blank sandbox browser, agents can directly control tabs in **your Personal Chrome**.

### 💡 Key Benefits for Everyday Users

- **Preserve Logged-in Sessions**: Work directly with services where you are already signed in (Google, GitHub, internal dashboards). You do not need to re-enter passwords.
- **Reliable Clicks on Protected Sites**: Input simulation works more reliably on sites with strict content security policies. Button clicks and keystrokes land as expected.
- **Save Raw Webpage HTML**: Saves complete webpage source files to your session workspace for offline inspection and verification.

| Execution Mode                         | Behavior                     | Best Suited For                                                    |
| -------------------------------------- | ---------------------------- | ------------------------------------------------------------------ |
| **Dedicated agent browser** (Default)  | Isolated LibrAgent browser   | Public web search, documentation research, sandbox tasks           |
| **Personal Chrome** (Extension Bridge) | Your everyday Chrome browser | Tasks requiring personal logins, private intranets, shopping carts |

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
   - The agent switches to Personal Chrome mode and controls your active tab.

> [!NOTE]
>
> - After installation, the extension reconnects automatically when you restart LibrAgent.
> - Each agent chat keeps exactly one active browser connection. Switching between the Dedicated agent browser and Personal Chrome closes the previous connection cleanly.

---

## 🌐 Key Capabilities

| Capability                   | Description                                                     |
| ---------------------------- | --------------------------------------------------------------- |
| **Page Navigation**          | Visits specified URLs and reads content once the page is ready. |
| **Screenshot Capture**       | Captures viewport or full-page images for visual inspection.    |
| **Console & Error Tracking** | Monitors page JavaScript errors and network state.              |
| **Clicks & Typing**          | Supports clicking links, typing text, and scrolling pages.      |

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
