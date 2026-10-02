# LibrAgent Browser Bridge (Chrome MV3)

Local **Load unpacked** extension that drives your everyday Chrome profile via a
loopback WebSocket to LibrAgent. This is an MVP — not published to the Chrome
Web Store.

## Load unpacked

1. Start LibrAgent (desktop app). The extension bridge listens on
   `ws://127.0.0.1:3847/extension-bridge` by default.
2. Open Chrome → `chrome://extensions` → enable **Developer mode**.
3. Click **Load unpacked** and select this folder
   (`chrome-extension/` in the LibrAgent repo).
4. Confirm the extension connects (service worker console should log
   `Connected to ws://127.0.0.1:3847/...`).
5. After updating this folder, click **Reload** on the extension card so
   permission / service-worker changes apply.

You can also copy the absolute path from **Settings → System → Agent browser →
Chrome extension bridge**.

## Permissions

| Permission | Why |
| --- | --- |
| `tabs` | Create / navigate / close agent tabs; history; visible screenshot |
| `scripting` | `evaluate` / page content / click / input (MAIN world) |
| `storage` / `alarms` | Options + keep the service worker / WebSocket alive |
| `<all_urls>` host access | Inject scripts and capture screenshots on sites the agent opens |

Granting `<all_urls>` lets LibrAgent control tabs the extension creates for the
agent. Prefer Load unpacked only on machines you trust.

## Defaults

| Setting | Default | Override |
| --- | --- | --- |
| Port | `3847` | Extension Options, or `LIBRAGENT_EXTENSION_BRIDGE_PORT` in the app |
| Token | `libragent-dev` | Extension Options, or `LIBRAGENT_EXTENSION_BRIDGE_TOKEN` in the app |

MVP auth: when the app env token is unset, LibrAgent uses the fixed dev token
`libragent-dev`. Production will use a rotating token.

## Backend selection (app)

| `LIBRAGENT_BROWSER_BACKEND` | Behavior |
| --- | --- |
| unset / `auto` | Prefer the extension when connected; otherwise sticky CDP sidecar |
| `extension` | Require the extension bridge |
| `sidecar` | Always use the sticky agent Chromium sidecar |

## Supported tools (via extension)

When Connected, these go through everyday Chrome:

- Session: `createSession`, `navigateToUrl`, `navigateBack`, `navigateForward`, `closeSession`
- Read / JS: `getPageTitle`, `getCurrentUrl`, `getPageContent`, `evaluateJS`, `listInteractable`
- Interact: `clickElement`, `inputText`, `scrollPage`
- Screenshot: `takeScreenshot` (**visible viewport only**; `fullPage` is ignored)

Still sidecar-only:

- `getConsoleLogs` (needs debugger-style hooks; deferred)

## Troubleshooting

- Settings shows **Disconnected**: open the extension service worker inspector or
  click the extension action, then Reload.
- Tools error with “not supported yet via Chrome extension bridge”: only
  `getConsoleLogs` should still say that after this parity work — reload the
  unpacked extension and restart LibrAgent if an older build is running.
