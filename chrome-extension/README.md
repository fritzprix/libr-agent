# LibrAgent Browser Bridge (Chrome MV3)

Companion extension that drives your everyday Chrome profile via a loopback
WebSocket to the LibrAgent desktop app.

## Install options

| Path | When to use |
| --- | --- |
| **Load unpacked** (below) | Development, contributors, pre-Store builds |
| **Chrome Web Store** | End users (unlisted/public once published) — see `STORE.md` |

Privacy policy: [`PRIVACY.md`](./PRIVACY.md). Store listing kit: [`STORE.md`](./STORE.md).

## Load unpacked

1. Start LibrAgent (desktop app). The extension bridge listens on
   `ws://127.0.0.1:3847/extension-bridge` by default.
2. Open Chrome → `chrome://extensions` → enable **Developer mode**.
3. Click **Load unpacked** and select this folder
   (`chrome-extension/` in the LibrAgent repo).
4. Confirm the extension connects (toolbar icon popup should say **Connected**,
   or the service worker console logs `Connected to ws://127.0.0.1:3847/...`).
5. After updating this folder (permissions / popup / icons), click **Reload** on
   the extension card once. Everyday LibrAgent restarts should **not** require
   revisiting this page — the extension reconnects automatically.

You can also copy the absolute path from **Settings → System → Agent browser →
Chrome extension bridge**.

## Package for Chrome Web Store

From the repo root:

```bash
pnpm chrome-extension:pack
```

Output: `dist/chrome-extension/libragent-browser-bridge-<version>.zip`
(`manifest.json` at ZIP root). Upload that file in the
[Chrome Developer Dashboard](https://chrome.google.com/webstore/devconsole).
Follow `STORE.md` for listing copy, privacy answers, and reviewer test steps.

## Permissions

| Permission | Why |
| --- | --- |
| `tabs` | Create / navigate / close agent tabs; history; visible screenshot |
| `scripting` | `evaluate` / page content / click / input (MAIN world) |
| `storage` / `alarms` | Options + keep the service worker / WebSocket alive |
| `<all_urls>` host access | Inject scripts and capture screenshots on sites the agent opens |

Granting `<all_urls>` lets LibrAgent control tabs the extension creates for the
agent. Only install from a source you trust.

## Defaults

| Setting | Default | Override |
| --- | --- | --- |
| Port | `3847` | Extension Options, or `LIBRAGENT_EXTENSION_BRIDGE_PORT` in the app |
| Token | `libragent-dev` | Extension Options, or `LIBRAGENT_EXTENSION_BRIDGE_TOKEN` in the app |

When the app env token is unset, LibrAgent uses the fixed development token
`libragent-dev`. A rotating production token is a follow-up for Store hardening.

## Backend selection (app)

`browser__createSession` takes an explicit `browser` argument:

| `browser` | Behavior |
| --- | --- |
| `sidecar` (default) | Sticky agent Chromium CDP sidecar |
| `userChrome` | Everyday Chrome via this extension — **errors if not Connected**; never falls back to sidecar |

`LIBRAGENT_BROWSER_BACKEND` remains for diagnostics/status only; it does not silently reroute createSession.

## Supported tools (via extension)

When Connected **and** the agent calls `createSession` with `browser="userChrome"`, these go through everyday Chrome:

- Session: `createSession`, `navigateToUrl`, `navigateBack`, `navigateForward`, `closeSession`
- Read / JS: `getPageTitle`, `getCurrentUrl`, `getPageContent`, `evaluateJS`, `listInteractable`
- Interact: `clickElement`, `inputText`, `scrollPage`
- Screenshot: `takeScreenshot` (**visible viewport only**; `fullPage` is ignored and logged)

Still sidecar-only:

- `getConsoleLogs` (needs debugger-style hooks; deferred)

## Troubleshooting

- Toolbar popup says **LibrAgent not running**: start the LibrAgent desktop app.
  Leave the extension enabled; it retries on its own.
- Toolbar popup says **Reconnecting…**: wait a few seconds after the app starts.
  You should not need `chrome://extensions` unless you changed extension files.
- After pulling extension code / permission changes: click **Reload** once on the
  extension card (developer update only).
- Tools error with “not supported yet via Chrome extension bridge”: only
  `getConsoleLogs` should still say that after tool parity — reload the unpacked
  extension and restart LibrAgent if an older build is running.
