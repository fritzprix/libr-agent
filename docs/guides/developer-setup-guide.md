# Developer Setup Guide

Welcome to the LibrAgent Developer Setup Guide. This document provides step-by-step instructions for getting your development environment up and running.

## System Requirements

- **Node.js**: Version 18 or higher.
- **pnpm**: Install globally with `npm install -g pnpm`.
- **Rust**: Required for native backend compilation. Follow instructions at [rustup.rs](https://rustup.rs/) (recommended for all platforms). Alternatively, on macOS/Linux: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`. **Important**: Restart your terminal after installing Rust to ensure the `cargo` command is available in your PATH.

### Linux Only System Dependencies
If you are developing on Debian/Ubuntu, you must install the following system dependencies before proceeding:

```bash
sudo apt-get update && sudo apt-get install -y libglib2.0-dev libgtk-3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

## Getting Started

1. **Fork and Clone**
   Fork the repository and clone your fork:
   ```bash
   git clone https://github.com/fritzprix/libr-agent
   cd libr-agent
   ```

2. **Install Dependencies**
   Use pnpm to install all required dependencies:
   ```bash
   pnpm install
   ```

3. **Run the Development Server**
   Start the Tauri development server locally. **Note:** There is no need to run `pnpm build` before running `pnpm tauri dev` because Tauri uses the Vite development server under the hood (`beforeDevCommand: pnpm dev`). Running `pnpm build` will just slow you down.

   ```bash
   pnpm tauri dev
   ```

   The first time you run this command, it will compile the entire Rust backend. This process can take several minutes depending on your hardware. Subsequent runs will be much faster. Once compiled, it will automatically open the LibrAgent desktop application.

## Troubleshooting

- **Rust/Cargo not found**: If you encounter errors about `cargo` not being found, ensure you have restarted your terminal after installing Rust.
- **Linux missing packages**: If the build fails on Linux complaining about missing headers or libraries (like `glib` or `webkit2gtk`), ensure you have run the `apt-get install` command listed in the System Requirements.
- **Node version errors**: Ensure you are using Node.js 18+ by running `node --version`.
