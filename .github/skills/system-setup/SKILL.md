---
name: system-setup
description: Guides installation and verification of MCP runtime dependencies (Python, Node.js, uv) across Windows, Linux, and macOS. Use when users need to set up their environment for running MCP servers or troubleshoot missing dependencies.
license: Complete terms in LICENSE.txt
---

# System Setup for MCP Servers

Automated installation and verification of MCP runtime dependencies.

## When to Use

- User reports MCP server failures due to missing runtimes
- Setting up LibrAgent on a new machine
- Troubleshooting "command not found" or "python/node not installed" errors
- Verifying system readiness for MCP operations

## Quick Start

### Check Current Installation

Use platform detection and verification commands:

**Windows (PowerShell):**
```powershell
Get-Command python -ErrorAction SilentlyContinue
Get-Command node -ErrorAction SilentlyContinue
Get-Command uv -ErrorAction SilentlyContinue
```

**Linux/macOS (Bash):**
```bash
which python3 && python3 --version
which node && node --version
which uv && uv --version
```

### Installation Strategy

1. **Detect OS** - Use `run_in_terminal` to identify platform
2. **Check existing installations** - Verify versions and PATH configuration
3. **Install missing components** - Follow OS-specific installation guides
4. **Verify installation** - Confirm executables are accessible

## Installation Guides

### Python Installation

**Windows:**
- Use Microsoft Store: `winget install Python.Python.3.12`
- Or download from python.org (ensure "Add to PATH" is checked)
- Verify: `python --version`

**Linux:**
```bash
# Ubuntu/Debian
sudo apt update && sudo apt install python3 python3-pip python3-venv

# Fedora/RHEL
sudo dnf install python3 python3-pip

# Arch Linux
sudo pacman -S python python-pip
```

**macOS:**
```bash
# Using Homebrew (recommended)
brew install python3

# Or use system Python (macOS 12.3+)
python3 --version
```

### Node.js Installation

**Windows:**
```powershell
# Using winget
winget install OpenJS.NodeJS.LTS

# Or download from nodejs.org
```

**Linux:**
```bash
# Ubuntu/Debian (NodeSource)
curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -
sudo apt-get install -y nodejs

# Fedora/RHEL
sudo dnf install nodejs npm

# Arch Linux
sudo pacman -S nodejs npm
```

**macOS:**
```bash
# Using Homebrew
brew install node
```

### uv Installation

**All Platforms:**
```bash
# Using pip (after Python is installed)
pip install uv

# Or using pipx (recommended for isolated installation)
pip install pipx
pipx install uv
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

After installation, verify all components:

```bash
# Python
python --version  # or python3 --version
pip --version

# Node.js
node --version
npm --version

# uv
uv --version
```

Expected output format:
- Python: `Python 3.11+`
- Node: `v18.0.0+`
- uv: `0.1.0+`

## Common Issues

### PATH Not Updated

**Symptoms:** `command not found` after installation

**Solutions:**
- **Windows:** Restart terminal or reboot system
- **Linux/macOS:** Run `source ~/.bashrc` or `source ~/.zshrc`
- Manually add to PATH if needed (see [PATH_CONFIG.md](references/PATH_CONFIG.md))

### Python vs Python3

**Linux/macOS:** Use `python3` and `pip3` explicitly
**Windows:** Usually aliased to `python` and `pip`

### Permission Issues

**Linux/macOS:** Use `sudo` for system-wide installation or pipx/venv for user-local
**Windows:** Run terminal as Administrator if needed

### Multiple Python Versions

Use virtual environments to isolate dependencies:
```bash
python -m venv mcp_env
source mcp_env/bin/activate  # Linux/macOS
.\mcp_env\Scripts\activate   # Windows
```

## Advanced Topics

- **PATH Configuration:** [PATH_CONFIG.md](references/PATH_CONFIG.md)
- **Virtual Environments:** [VENV_GUIDE.md](references/VENV_GUIDE.md)
- **Offline Installation:** [OFFLINE_INSTALL.md](references/OFFLINE_INSTALL.md)

## Scripts

Automated installation scripts are available in `scripts/`:

- `install_python.ps1` / `install_python.sh` - Python installation
- `install_node.ps1` / `install_node.sh` - Node.js installation  
- `install_uv.ps1` / `install_uv.sh` - uv installation
- `verify_setup.ps1` / `verify_setup.sh` - Comprehensive verification

Execute scripts based on user's platform and permission level.

## Integration with LibrAgent

LibrAgent MCP servers require:
- **Python 3.11+** for Python-based MCP servers
- **Node.js 18+** for TypeScript-based MCP servers
- **uv** for fast Python dependency management

Check `src-tauri/src/mcp/server_manager.rs` for runtime detection logic.
