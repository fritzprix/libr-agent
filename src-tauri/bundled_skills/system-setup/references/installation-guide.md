# Installation Guide

## Python

**Windows:**

```powershell
winget install Python.Python.3.12
# Verify: python --version (ensure "Add to PATH" if installing from python.org)
```

**Linux:**

```bash
# Ubuntu/Debian
sudo apt update && sudo apt install python3 python3-pip python3-venv
# Fedora/RHEL: sudo dnf install python3 python3-pip
# Arch: sudo pacman -S python python-pip
```

**macOS:**

```bash
brew install python3
# Or use system Python (macOS 12.3+): python3 --version
```

## Node.js

**Windows:**

```powershell
winget install OpenJS.NodeJS.LTS
```

**Linux:**

```bash
# Ubuntu/Debian (NodeSource)
curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -
sudo apt-get install -y nodejs
# Fedora/RHEL: sudo dnf install nodejs npm
# Arch: sudo pacman -S nodejs npm
```

**macOS:**

```bash
brew install node
```

## uv

**pip (after Python):**

```bash
pip install uv
# Or isolated: pip install pipx && pipx install uv
```

**Windows PowerShell (standalone):**

```powershell
irm https://astral.sh/uv/install.ps1 | iex
```

**Linux/macOS (standalone):**

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
```

## Verification

```bash
python --version   # or python3
pip --version
node --version
npm --version
uv --version
```

Expected: Python 3.11+, Node v18+, uv 0.1.0+

## Common Issues

### PATH not updated

- **Windows**: restart terminal or reboot
- **Linux/macOS**: `source ~/.bashrc` or `source ~/.zshrc`

### Python vs python3

Linux/macOS: prefer `python3` / `pip3`. Windows: usually `python` / `pip`.

### Permission issues

Linux/macOS: `sudo` for system-wide, or pipx/venv for user-local. Windows: Administrator if needed.

### Multiple Python versions

```bash
python -m venv mcp_env
source mcp_env/bin/activate   # Linux/macOS
.\mcp_env\Scripts\activate    # Windows
```

## Advanced

- PATH configuration: see PATH_CONFIG.md (if present in skill bundle)
- Virtual environments: VENV_GUIDE.md
- Offline installation: OFFLINE_INSTALL.md

## LibrAgent Requirements

- Python 3.11+ for Python-based MCP servers
- Node.js 18+ for TypeScript-based MCP servers
- uv for fast Python dependency management
