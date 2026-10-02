# Privacy Policy — LibrAgent Browser Bridge

**Last updated:** 2026-10-03

This privacy policy applies to the **LibrAgent Browser Bridge** Chrome extension
(the “Extension”).

## Who we are

LibrAgent is a desktop AI agent application. The Extension is a companion that
lets the LibrAgent app on **your computer** control Chrome tabs that you allow
it to use.

Publisher contact: see the GitHub repository
[fritzprix/libr-agent](https://github.com/fritzprix/libr-agent).

## Single purpose

The Extension’s only purpose is to bridge the local LibrAgent desktop app to
Chrome so agents can create, navigate, read, and interact with browser tabs on
this machine.

## Data the Extension handles

The Extension:

- Opens a **local** WebSocket to LibrAgent on `127.0.0.1` (default port `3847`).
- Creates and controls tabs at the direction of the local LibrAgent app.
- May read page content, run page scripts, capture **viewport** screenshots, and
  send results back to the local app over that loopback connection.

The Extension does **not**:

- Send browsing data to LibrAgent cloud servers by itself.
- Sell user data.
- Use remote code hosted outside the extension package for its logic.
- Collect analytics or advertising identifiers.

Page content and screenshots only leave your machine if **you** configure the
LibrAgent app to send them to a model provider or other service you choose. That
behavior is governed by LibrAgent and your chosen providers, not by this
Extension’s network traffic (which stays on localhost).

## Permissions

| Permission | Why |
| --- | --- |
| `tabs` | Create, navigate, close, and inspect agent-driven tabs |
| `scripting` | Read page content and perform clicks/typing when the agent requests it |
| `storage` | Save bridge port/token and UI connection status |
| `alarms` | Keep the local bridge connection reliable while LibrAgent is running |
| Host access (`<all_urls>`) | Agents may open arbitrary URLs the user asks for; the Extension must reach those pages |

## Authentication

The bridge uses a shared token (default development token `libragent-dev` unless
overridden in Extension Options / LibrAgent environment). Only processes that
know the token can talk to the local bridge.

## Data retention

The Extension stores small configuration values (port, token, connection status)
in `chrome.storage.local` on your device. You can remove them by uninstalling the
Extension or clearing extension storage.

## Changes

We may update this policy when the Extension’s behavior changes. The “Last
updated” date at the top will change when we do.

## Contact

Open an issue on
[github.com/fritzprix/libr-agent](https://github.com/fritzprix/libr-agent/issues).
