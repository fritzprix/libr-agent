---
name: setup-wizard
description: Use when the user needs to diagnose and install runtime environment dependencies (Python, Node.js, etc.) for running LibrAgent. LibrAgent 실행에 필요한 Python, Node.js 등의 환경을 자동으로 진단하고 설치하도록 안내하는 마법사입니다.
---

# System Setup for MCP Servers

Diagnose missing runtimes and guide installation—do not dump full OS install guides into the conversation.

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
