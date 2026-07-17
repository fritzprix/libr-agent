---
description: Build production frontend + Tauri app
---

Build LibrAgent for production distribution.

Command: `pnpm tauri build --config '{"bundle":{"createUpdaterArtifacts":false}}'`

This produces platform-specific bundles in `src-tauri/target/release/bundle/`.
