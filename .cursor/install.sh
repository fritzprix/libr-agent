#!/usr/bin/env bash
#
# Cloud Agent install script for LibrAgent.
#
# LibrAgent is a Tauri 2 desktop app: a Rust backend (src-tauri/) plus a
# React + Vite + TypeScript frontend (src/). This script prepares a Linux VM
# so both halves compile, test, and run. It is idempotent and safe to re-run.
set -euo pipefail

echo "==> [1/3] Installing system libraries for the Tauri (Rust) backend"
export DEBIAN_FRONTEND=noninteractive
sudo apt-get update
# WebKitGTK / GTK stack is required to compile and link the Tauri backend on
# Linux. g++-14 provides the libstdc++ development symlink that clang (the
# default `cc`) selects when linking C++ dependencies such as onig/aws-lc;
# build-essential provides the matching gcc/g++-13 dev libraries as a fallback.
sudo apt-get install -y --no-install-recommends \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libglib2.0-dev \
  libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libxdo-dev \
  libssl-dev \
  patchelf \
  pkg-config \
  build-essential \
  g++-14 \
  file \
  curl \
  wget

echo "==> [2/3] Ensuring an up-to-date stable Rust toolchain"
# The dependency tree pulls crates that use the 2024 edition, which needs
# cargo >= 1.85. Install/refresh stable with the lint components CI uses.
if command -v rustup >/dev/null 2>&1; then
  rustup toolchain install stable --profile minimal --component clippy --component rustfmt --no-self-update
  rustup default stable
else
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain stable --profile minimal --component clippy rustfmt
  # shellcheck disable=SC1090
  . "$HOME/.cargo/env"
fi

echo "==> [3/3] Installing JavaScript dependencies (pnpm, frozen lockfile)"
# package.json pins pnpm@9.15.9 and a preinstall hook enforces it.
corepack enable
corepack prepare pnpm@9.15.9 --activate
pnpm install --frozen-lockfile

echo "==> Install complete."
