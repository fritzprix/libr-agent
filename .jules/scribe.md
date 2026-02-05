# Scribe's Journal - Drift Log

## 2024-05-22 - README.md
**Drift:** Linux installation instructions for building from source were missing critical system dependencies (`libglib2.0-dev`, `libgtk-3-dev`, etc.), causing `cargo test` to fail.
**Reality:** Users must install `libglib2.0-dev libgtk-3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libwebkit2gtk-4.1-dev` on Debian/Ubuntu to build the Tauri backend.

## 2024-05-22 - src/README.md
**Drift:** Contains Python code inside `js` code blocks. Contains typos ("OpneAI"). References potentially non-existent models (`gpt-4.1`, `o4-mini`).
**Reality:** Documentation should use correct language tags and verified model names (e.g., `gpt-4o`, `gpt-4o-mini`).
