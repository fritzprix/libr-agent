---
name: setup-wizard
description: 'Use when the user needs to diagnose and install runtime environment dependencies (Python, Node.js, uv) for running LibrAgent or MCP servers, or when a task fails because a runtime is missing. Use on first run or a new machine ("I just installed LibrAgent, what do I need to set up?"), MCP "command not found" or spawn errors, exit code 127, a missing Python or Node version (python3.9 not found), a broken venv, or os error 448 (ERROR_UNTRUSTED_MOUNT_POINT). Triggers: 처음 깔았는데 뭐 설정해야 돼, 초기 설정, 환경 설정, command not found, exit code 127, node 없음, python 없음, 파이썬 버전 없음, venv broken, os error 448. LibrAgent 실행에 필요한 Python, Node.js 등의 환경을 자동으로 진단하고 설치하도록 안내하는 마법사입니다.'
license: Complete terms in LICENSE.txt
---

# System Setup for MCP Servers

Automated installation and verification of MCP runtime dependencies.

## Path conventions

Paths in this skill are relative to this skill's Base Directory. Replace `<skill-base-dir>` with its absolute path in commands.

## Missing runtime during a task (fixed order)

Use this when a command fails with `command not found`, exit code 127, a missing Python or Node version, or a broken venv:

1. Look for a user-scope tool first: `uv`, conda, nvm (`Get-Command uv` / `command -v uv conda`; nvm is a shell function, so check `$NVM_DIR`).
2. For Python, use `uv run --python <version> <script.py>` or `uv python install <version>`.
3. Do **not** use `sudo` or a system package manager (`apt`, `dnf`, `winget --scope machine`). If a step really needs root, use only `sudo -n`. If `sudo -n` fails, report to the user.
4. After **one** install attempt, run the check again. If it still fails, report the error to the user and stop.

If the output contains `os error 448` or `ERROR_UNTRUSTED_MOUNT_POINT`, stop and report: Windows RedirectionGuard blocks the uv Python junction (#2527). Do not reinstall.

## Workflow

1. **Detect OS** — identify platform via terminal
2. **Check installations** — run verification commands below
3. **Install missing components** — use platform scripts or [installation-guide.md](references/installation-guide.md)
4. **Verify** — run `verify_setup` script or manual checks; confirm Python 3.11+, Node 18+, uv available

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

## Automated Scripts

Prefer scripts when the user wants hands-off install:

```powershell
# Windows examples
& "<skill-base-dir>/scripts/verify_setup.ps1"
& "<skill-base-dir>/scripts/install_python.ps1"
& "<skill-base-dir>/scripts/install_node.ps1"
& "<skill-base-dir>/scripts/install_uv.ps1"
```

```bash
# Linux/macOS examples (install_node.sh is Linux-only; on macOS use brew install node)
bash "<skill-base-dir>/scripts/verify_setup.sh"
bash "<skill-base-dir>/scripts/install_python.sh"
bash "<skill-base-dir>/scripts/install_node.sh"   # Linux only; no-op if node exists, add --force to reinstall
bash "<skill-base-dir>/scripts/install_uv.sh"
```

## Advanced Topics

- **PATH configuration:** [PATH_CONFIG.md](references/PATH_CONFIG.md)
- **Virtual environments:** [VENV_GUIDE.md](references/VENV_GUIDE.md)
- **Offline installation:** [OFFLINE_INSTALL.md](references/OFFLINE_INSTALL.md)
- **OS-specific commands:** [installation-guide.md](references/installation-guide.md)

## Integration with LibrAgent

LibrAgent MCP servers require:

- **Python 3.11+** for Python-based MCP servers
- **Node.js 18+** for TypeScript-based MCP servers
- **uv** for fast Python dependency management

After setup, re-run the verification script before retrying failed MCP servers.
