---
description: One-shot environment setup for new machines
---

One-shot environment setup for LibrAgent development.

1. Enable corepack: `corepack enable`
2. Pin pnpm: `corepack prepare pnpm@9.15.9 --activate`
3. Install dependencies: `pnpm install --frozen-lockfile`
4. Verify Rust toolchain: `rustup` installed and stable target available
5. Platform dependency checks via `pnpm diagnose`

Use the `setup-wizard` skill for automated environment diagnosis.
