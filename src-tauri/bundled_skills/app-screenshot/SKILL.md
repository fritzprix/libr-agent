---
name: app-screenshot
description: Cross-platform live desktop app window capture skill for LibrAgent. Supports Windows and Linux. Triggers on requests like '앱 화면 캡처', '앱 스크린샷', 'capture app window', 'app screenshot', 'capture ui'.
---

# App Screenshot Skill

Capture live, pixel-exact desktop screenshots of the running LibrAgent application window across **Windows and Linux**.

## Overview

Use this skill to:
- Capture live screenshots of the LibrAgent desktop app (`/usr/bin/libragent` or `tauri dev`)
- Update documentation UI assets under `docs/user/assets/screenshots/`
- Visually verify UI layout, routes, or interactive components

## OS Support

| Platform | Window Search Mechanism | Capture Engine |
| :--- | :--- | :--- |
| **Windows** | Win32 `EnumWindows` + `GetWindowRect` (`user32.dll`) | Python `mss` + `PIL` |
| **Linux** | X11 `xwininfo -root -tree` + `_NET_ACTIVE_WINDOW` | Python `mss` + `PIL` |

## Usage

### Direct Script Execution

Run the built-in cross-platform Python helper script:

```bash
python src-tauri/bundled_skills/app-screenshot/scripts/capture_app.py <OUTPUT_PATH> [WINDOW_TITLE_SUBSTRING]
```

Example:

```bash
python src-tauri/bundled_skills/app-screenshot/scripts/capture_app.py docs/user/assets/screenshots/getting-started/new-session.png LibrAgent
```

### Post-Capture Verification

After updating screenshot assets:
1. Verify document link references in `docs/user/` and `docs/user/en/`.
2. Run VitePress build check:

```bash
pnpm docs:build
```
