---
description: Run system diagnostics for desktop runtime issues
---

Run diagnostics for LibrAgent desktop runtime issues.

Command: `pnpm diagnose`

Performs platform-specific checks:

- Linux: WebKit/GTK dependencies
- macOS: SDKROOT and code signing prerequisites
- Windows: DLL dependencies, WebView2 runtime

Use this when the app fails to launch or displays a blank white screen.
