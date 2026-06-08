---
name: system-setup
description: Guides installation and verification of MCP runtime dependencies (Python, Node.js, uv) across Windows, Linux, and macOS. Use when users need to set up their environment for running MCP servers or troubleshoot missing dependencies.
license: Complete terms in LICENSE.txt
---

# System Setup for MCP Servers

Diagnose missing runtimes and guide installation—do not dump full OS install guides into the conversation.

## When to Use

- MCP server failures due to missing runtimes
- New machine setup for LibrAgent
- "command not found" for python, node, or uv

## Workflow

1. **Detect OS** — identify platform via terminal
2. **Check installations** — run verification commands below
3. **Install missing components** — follow [references/installation-guide.md](references/installation-guide.md) for the detected OS only
4. **Verify** — re-run checks; confirm Python 3.11+, Node 18+, uv available

## Quick Verification

**Windows (PowerShell):**

```powershell
Get-Command python -ErrorAction SilentlyContinue
Get-Command node -ErrorAction SilentlyContinue
Get-Command uv -ErrorAction SilentlyContinue
```

**Linux/macOS:**

```bash
which python3 && python3 --version
which node && node --version
which uv && uv --version
```

Report what is missing, then load only the relevant section from the installation guide.

## Scripts

Platform scripts in `scripts/` (if bundled): `install_python`, `install_node`, `install_uv`, `verify_setup` — use when the user prefers automated install over manual commands.
